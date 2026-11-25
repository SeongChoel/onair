//! onair 서비스: 모든 규칙을 가진다. 서버 함수와 HTTP 핸들러는 이것을 부르는 얇은 통로다.
use std::path::{Path, PathBuf};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

pub use crate::media::{MediaMtxClient, MediaServer, NoMediaServer};
use crate::{
    ChannelStatus, ChannelSummary, ChatMessage, ChatServerMsg, StudioChannel, MAX_NICKNAME_CHARS,
};
use tokio::sync::broadcast;

const SCHEMA: &str = include_str!("schema.sql");

/// 테스트 채널 이름과 송출 키. 키는 고정값이며 docs/dev.md에 적혀 있다.
const SEED_CHANNEL_NAMES: [&str; 3] = ["테스트 채널 1", "테스트 채널 2", "테스트 채널 3"];
pub const SEED_STREAM_KEYS: [&str; 3] = ["test-key-1", "test-key-2", "test-key-3"];

/// 채널과 진행 중인 방송 제목(`live_title`)을 함께 읽는 쿼리 머리.
macro_rules! select_summary {
    ($tail:literal) => {
        concat!(
            "SELECT c.id, c.name, c.profile_image,
    (SELECT b.title FROM broadcasts b WHERE b.channel_id = c.id AND b.ended_at IS NULL) AS live_title
    FROM channels c ",
            $tail
        )
    };
}

fn to_summary(r: &sqlx::sqlite::SqliteRow) -> ChannelSummary {
    let image: Option<String> = r.get("profile_image");
    ChannelSummary {
        id: r.get("id"),
        name: r.get("name"),
        profile_image: image.map(|f| format!("/uploads/{f}")),
        live_title: r.get("live_title"),
    }
}

/// 무작위 영숫자 `len`자.
fn random_alphanumeric(len: usize) -> String {
    use rand::distr::{Alphanumeric, SampleString};
    Alphanumeric.sample_string(&mut rand::rng(), len)
}

/// 송출 키: 고정 접두어 + 무작위 영숫자 32자. RTMP 주소의 쿼리에 그대로 실린다.
fn new_stream_key() -> String {
    format!("onair_{}", random_alphanumeric(32))
}

const MAX_CHAT_CHARS: usize = 500;
const RECENT_CHAT_LIMIT: i64 = 50;

/// 도배 방지: 같은 채널·같은 닉네임으로 `CHAT_FLOOD_WINDOW` 안에 `CHAT_FLOOD_MAX`개까지만 보낸다.
const CHAT_FLOOD_MAX: usize = 5;
const CHAT_FLOOD_WINDOW: Duration = Duration::from_secs(5);
const CHAT_FLOOD_REASON: &str = "잠시 후 다시 보내세요";

/// (채널 아이디, 닉네임)별로 최근에 보낸 시각들.
type ChatSendTimes = Mutex<HashMap<(i64, String), VecDeque<Instant>>>;

type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

fn to_chat_message(r: &sqlx::sqlite::SqliteRow) -> ChatMessage {
    ChatMessage {
        id: r.get("id"),
        nickname: r.get("nickname"),
        content: r.get("content"),
        sent_at: r.get("sent_at"),
    }
}

const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

/// 파일 내용의 앞부분(매직 바이트)으로 형식을 판별해 확장자를 돌려준다.
fn image_extension(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// 방송 하나의 채팅 구독. 구독한 시점의 최근 메시지(`recent`)와 그 뒤의 새 메시지·종료를 받는다.
pub struct ChatSubscription {
    /// 구독 시점의 최근 메시지(오래된 순, 최대 50개).
    pub recent: Vec<ChatMessage>,
    rx: broadcast::Receiver<ChatServerMsg>,
    /// `recent`에 이미 있는 마지막 메시지 아이디. 그 이하는 다시 주지 않는다.
    last_seen: i64,
    ended: bool,
}

impl ChatSubscription {
    /// 다음 메시지. `Ended`를 한 번 준 뒤에는 `None`이다.
    pub async fn recv(&mut self) -> Option<ChatServerMsg> {
        loop {
            if self.ended {
                return None;
            }
            match self.rx.recv().await {
                Ok(ChatServerMsg::Message(m)) if m.id <= self.last_seen => continue,
                Ok(msg) => {
                    self.ended = msg == ChatServerMsg::Ended;
                    return Some(msg);
                }
                // 너무 느려 일부를 놓쳤다. 남은 것부터 이어 받는다.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

/// 진행 중인 방송 아이디별 채팅 전달 통로.
type BroadcastChats = Mutex<HashMap<i64, broadcast::Sender<ChatServerMsg>>>;

/// 채널의 진행 중인 방송 아이디. 없으면 `None`.
async fn live_broadcast_id<'e>(
    db: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    channel_id: i64,
) -> Result<Option<i64>, String> {
    sqlx::query_scalar("SELECT id FROM broadcasts WHERE channel_id = ? AND ended_at IS NULL")
        .bind(channel_id)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())
}

pub struct OnairService {
    pool: SqlitePool,
    chat: BroadcastChats,
    chat_send_times: ChatSendTimes,
    clock: Clock,
    upload_dir: PathBuf,
    media: Arc<dyn MediaServer>,
}

impl OnairService {
    /// SQLite 파일과 업로드 폴더 위치를 받아 서비스를 만든다. 없으면 만든다.
    pub async fn open(
        db_path: impl AsRef<Path>,
        upload_dir: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        Self::open_with_media(db_path, upload_dir, Arc::new(NoMediaServer)).await
    }

    /// `open`에 영상 서버(MediaMTX 또는 테스트용 가짜)를 함께 받는다.
    pub async fn open_with_media(
        db_path: impl AsRef<Path>,
        upload_dir: impl Into<PathBuf>,
        media: Arc<dyn MediaServer>,
    ) -> Result<Self, String> {
        let upload_dir = upload_dir.into();
        std::fs::create_dir_all(&upload_dir).map_err(|e| e.to_string())?;
        if let Some(parent) = db_path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        let service = Self {
            pool,
            chat: Mutex::default(),
            chat_send_times: Mutex::default(),
            clock: Arc::new(Instant::now),
            upload_dir,
            media,
        };
        service.seed_if_empty().await?;
        Ok(service)
    }

    /// 도배 방지에 쓰는 시계를 바꾼다. 테스트에서 시간을 직접 움직일 때 쓴다.
    pub fn with_clock(mut self, clock: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// 도배 방지: 창 안에 이미 `CHAT_FLOOD_MAX`개를 보냈으면 거부하고, 아니면 이번 시각을 기록한다.
    /// 거부된 시도는 기록하지 않는다.
    fn record_chat_send(&self, channel_id: i64, nickname: &str) -> Result<(), String> {
        let now = (self.clock)();
        let mut times = self.chat_send_times.lock().unwrap();
        let sent = times.entry((channel_id, nickname.to_string())).or_default();
        while sent.front().is_some_and(|t| now.duration_since(*t) >= CHAT_FLOOD_WINDOW) {
            sent.pop_front();
        }
        if sent.len() >= CHAT_FLOOD_MAX {
            return Err(CHAT_FLOOD_REASON.to_string());
        }
        sent.push_back(now);
        Ok(())
    }

    pub fn upload_dir(&self) -> &Path {
        &self.upload_dir
    }

    /// 연결을 닫는다. 테스트가 SQLite 파일을 지우기 전에 쓴다.
    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn list_channels(&self) -> Result<Vec<ChannelSummary>, String> {
        let rows = sqlx::query(select_summary!("ORDER BY live_title IS NULL, c.id"))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(to_summary).collect())
    }

    /// 채널 하나. 없으면 사용자에게 보여 줄 수 있는 이유로 실패한다.
    pub async fn get_channel(&self, id: i64) -> Result<ChannelSummary, String> {
        let row = sqlx::query(select_summary!("WHERE c.id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        row.as_ref()
            .map(to_summary)
            .ok_or_else(|| "없는 채널입니다".to_string())
    }

    /// 채널의 방송·영상 신호 상태. 영상 신호는 방송 중일 때만 영상 서버에 묻고,
    /// 영상 서버에 물을 수 없으면 신호 없음으로 본다. 신호가 끊겨도 방송은 종료하지 않는다.
    pub async fn channel_status(&self, id: i64) -> Result<ChannelStatus, String> {
        self.get_channel(id).await?; // 없는 채널이면 여기서 실패한다
        // 제목과 아이디는 한 번에 읽어 같은 방송의 것이 되게 한다.
        let live: Option<(i64, String)> = sqlx::query_as(
            "SELECT id, title FROM broadcasts WHERE channel_id = ? AND ended_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        let Some((broadcast_id, live_title)) = live else {
            return Ok(ChannelStatus {
                live_title: None,
                broadcast_id: None,
                has_signal: false,
            });
        };
        let has_signal = self.media.has_signal(id).await.unwrap_or_else(|e| {
            eprintln!("영상 신호 확인 실패(채널 {id}): {e}");
            false
        });
        Ok(ChannelStatus {
            live_title: Some(live_title),
            broadcast_id: Some(broadcast_id),
            has_signal,
        })
    }

    /// 채널 만들기. 이름은 앞뒤 공백을 자르며, 비었거나 이미 있으면 이유와 함께 실패한다.
    pub async fn create_channel(&self, name: &str) -> Result<StudioChannel, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("채널 이름을 입력하세요".to_string());
        }
        let stream_key = new_stream_key();
        let result = sqlx::query("INSERT INTO channels (name, stream_key) VALUES (?, ?)")
            .bind(name)
            .bind(&stream_key)
            .execute(&self.pool)
            .await;
        match result {
            Ok(done) => Ok(StudioChannel {
                id: done.last_insert_rowid(),
                name: name.to_string(),
                stream_key,
                live_title: None,
            }),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                Err("이미 있는 채널 이름입니다".to_string())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// 스튜디오 들어가기. 송출 키가 없으면 실패한다.
    pub async fn enter_studio(&self, stream_key: &str) -> Result<StudioChannel, String> {
        let row = sqlx::query(
            "SELECT c.id, c.name, c.stream_key,
                (SELECT b.title FROM broadcasts b WHERE b.channel_id = c.id AND b.ended_at IS NULL) AS live_title
             FROM channels c WHERE c.stream_key = ?",
        )
            .bind(stream_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        row.map(|r| StudioChannel {
            id: r.get("id"),
            name: r.get("name"),
            stream_key: r.get("stream_key"),
            live_title: r.get("live_title"),
        })
        .ok_or_else(|| crate::WRONG_STREAM_KEY.to_string())
    }

    /// 프로필 이미지 올리기. 송출 키가 맞아야 하고, 내용이 jpg/png/webp이며 2MB 이하여야 한다.
    /// 새 파일을 저장해 채널에 연결한 뒤 이전 파일을 지운다.
    pub async fn upload_profile_image(
        &self,
        stream_key: &str,
        bytes: &[u8],
    ) -> Result<ChannelSummary, String> {
        let row = sqlx::query("SELECT id, profile_image FROM channels WHERE stream_key = ?")
            .bind(stream_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| crate::WRONG_STREAM_KEY.to_string())?;
        let id: i64 = row.get("id");
        let old: Option<String> = row.get("profile_image");

        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("이미지는 2MB 이하여야 합니다".to_string());
        }
        let ext = image_extension(bytes)
            .ok_or_else(|| "jpg, png, webp 이미지만 올릴 수 있습니다".to_string())?;

        // 파일 이름에 송출 키를 쓰지 않는다. 이름이 바뀌어야 브라우저 캐시도 새로 받는다.
        let file_name = format!("channel-{id}-{}.{ext}", random_alphanumeric(12));
        std::fs::write(self.upload_dir.join(&file_name), bytes).map_err(|e| e.to_string())?;
        let updated = sqlx::query("UPDATE channels SET profile_image = ? WHERE id = ?")
            .bind(&file_name)
            .bind(id)
            .execute(&self.pool)
            .await;
        if let Err(e) = updated {
            let _ = std::fs::remove_file(self.upload_dir.join(&file_name));
            return Err(e.to_string());
        }
        if let Some(old) = old {
            let _ = std::fs::remove_file(self.upload_dir.join(old));
        }
        self.get_channel(id).await
    }

    /// 방송 시작. 제목이 비면 "{채널 이름}의 방송". 진행 중인 방송이 있으면 거부한다. 정해진 제목을 돌려준다.
    pub async fn start_broadcast(&self, stream_key: &str, title: &str) -> Result<String, String> {
        let channel = self.enter_studio(stream_key).await?;
        let title = title.trim();
        let title = if title.is_empty() {
            format!("{}의 방송", channel.name)
        } else {
            title.to_string()
        };
        let result = sqlx::query("INSERT INTO broadcasts (channel_id, title) VALUES (?, ?)")
            .bind(channel.id)
            .bind(&title)
            .execute(&self.pool)
            .await;
        match result {
            Ok(_) => Ok(title),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                Err("이미 진행 중인 방송이 있습니다".to_string())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// 방송 종료. 진행 중인 방송에 종료 시각을 기록한다. 없으면 거부한다.
    pub async fn end_broadcast(&self, stream_key: &str) -> Result<(), String> {
        let channel = self.enter_studio(stream_key).await?;
        // 방송 아이디 확인과 종료 기록은 한 트랜잭션이다.
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let broadcast_id = live_broadcast_id(&mut *tx, channel.id)
            .await?
            .ok_or_else(|| "진행 중인 방송이 없습니다".to_string())?;
        sqlx::query("UPDATE broadcasts SET ended_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(broadcast_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        // 구독 쪽에 종료를 알리고 통로를 닫는다. 종료를 기록한 뒤에 하므로,
        // 이 뒤에 들어오는 구독은 `subscribe_chat`의 재확인에서 걸러진다.
        let sender = self.chat.lock().unwrap().remove(&broadcast_id);
        if let Some(sender) = sender {
            let _ = sender.send(ChatServerMsg::Ended);
        }
        // 이미 연결된 송출도 끊는다. 실패해도 종료는 유효하다(OBS가 다시 붙으면 허락 확인에서 거부된다).
        if let Err(e) = self.media.kick_publisher(channel.id).await {
            eprintln!("송출 끊기 실패(채널 {}): {e}", channel.id);
        }
        Ok(())
    }

    /// 채팅 보내기. 채널의 진행 중인 방송에 저장한다. 닉네임은 앞뒤 공백을 자르며 1~20자,
    /// 내용은 비어 있지 않고 500자 이하여야 한다. 진행 중인 방송이 없으면 거부한다.
    /// 저장한 직후 그 방송의 구독자에게 전달한다.
    pub async fn send_chat(
        &self,
        channel_id: i64,
        nickname: &str,
        content: &str,
    ) -> Result<ChatMessage, String> {
        let nickname = nickname.trim();
        if nickname.is_empty() {
            return Err("닉네임을 입력하세요".to_string());
        }
        if nickname.chars().count() > MAX_NICKNAME_CHARS {
            return Err("닉네임은 20자 이하여야 합니다".to_string());
        }
        if content.trim().is_empty() {
            return Err("메시지를 입력하세요".to_string());
        }
        if content.chars().count() > MAX_CHAT_CHARS {
            return Err("메시지는 500자 이하여야 합니다".to_string());
        }
        if live_broadcast_id(&self.pool, channel_id).await?.is_none() {
            return Err("방송 중이 아니라 채팅할 수 없습니다".to_string());
        }
        self.record_chat_send(channel_id, nickname)?;
        let row = sqlx::query(
            "INSERT INTO chat_messages (broadcast_id, nickname, content)
             SELECT b.id, ?, ? FROM broadcasts b WHERE b.channel_id = ? AND b.ended_at IS NULL
             RETURNING id, broadcast_id, nickname, content, sent_at",
        )
        .bind(nickname)
        .bind(content)
        .bind(channel_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "방송 중이 아니라 채팅할 수 없습니다".to_string())?;
        let message = to_chat_message(&row);
        let sender = self.chat.lock().unwrap().get(&row.get::<i64, _>("broadcast_id")).cloned();
        if let Some(sender) = sender {
            // 받는 쪽이 없으면 실패하지만 저장은 이미 끝났다.
            let _ = sender.send(ChatServerMsg::Message(message.clone()));
        }
        Ok(message)
    }

    /// 채팅 구독. 채널의 진행 중인 방송을 구독하고, 구독을 시작한 뒤에 최근 50개를 읽는다.
    /// 그래서 읽는 사이 저장된 메시지도 놓치지 않으며, 겹치는 것은 `recv`가 걸러 낸다.
    /// 방송 중이 아니면 거부한다.
    pub async fn subscribe_chat(&self, channel_id: i64) -> Result<ChatSubscription, String> {
        const NOT_LIVE: &str = "방송 중이 아니라 채팅을 받을 수 없습니다";
        let broadcast_id = live_broadcast_id(&self.pool, channel_id)
            .await?
            .ok_or_else(|| NOT_LIVE.to_string())?;
        let rx = self
            .chat
            .lock()
            .unwrap()
            .entry(broadcast_id)
            .or_insert_with(|| broadcast::channel(256).0)
            .subscribe();
        // 통로에 들어선 뒤 방송이 아직 진행 중인지 다시 본다. 그 사이 종료됐다면 `Ended`를 받을 길이
        // 없으므로 거부하고, 종료 쪽이 이미 치운 통로를 되살렸으니 다시 치운다.
        if live_broadcast_id(&self.pool, channel_id).await? != Some(broadcast_id) {
            self.chat.lock().unwrap().remove(&broadcast_id);
            return Err(NOT_LIVE.to_string());
        }
        let recent = self.recent_chat(channel_id).await?;
        let last_seen = recent.last().map_or(0, |m| m.id);
        Ok(ChatSubscription {
            recent,
            rx,
            last_seen,
            ended: false,
        })
    }

    /// 최근 채팅. 진행 중인 방송의 마지막 50개를 오래된 순으로 돌려준다. 방송 중이 아니면 비어 있다.
    pub async fn recent_chat(&self, channel_id: i64) -> Result<Vec<ChatMessage>, String> {
        let rows = sqlx::query(
            "SELECT id, nickname, content, sent_at FROM (
                SELECT m.id, m.nickname, m.content, m.sent_at FROM chat_messages m
                JOIN broadcasts b ON b.id = m.broadcast_id
                WHERE b.channel_id = ? AND b.ended_at IS NULL
                ORDER BY m.id DESC LIMIT ?
             ) ORDER BY id",
        )
        .bind(channel_id)
        .bind(RECENT_CHAT_LIMIT)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(to_chat_message).collect())
    }

    /// 영상 수신 허락. MediaMTX가 넘긴 동작·경로·쿼리로 판단한다.
    /// 송출은 경로가 `live/<채널 아이디>`이고 쿼리 `key`가 그 채널의 송출 키이며
    /// 그 채널에 진행 중인 방송이 있을 때만 허락한다. 재생은 항상 허락한다.
    pub async fn authorize_ingest(&self, action: &str, path: &str, query: &str) -> bool {
        match action {
            "read" | "playback" => true,
            "publish" => self.may_publish(path, query).await,
            _ => false,
        }
    }

    async fn may_publish(&self, path: &str, query: &str) -> bool {
        let Some(id) = path
            .strip_prefix("live/")
            .filter(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|rest| rest.parse::<i64>().ok())
        else {
            return false;
        };
        let Some(key) = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("key="))
            .filter(|k| !k.is_empty())
        else {
            return false;
        };
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM channels c WHERE c.id = ? AND c.stream_key = ?
               AND EXISTS (SELECT 1 FROM broadcasts b WHERE b.channel_id = c.id AND b.ended_at IS NULL)",
        )
        .bind(id)
        .bind(key)
        .fetch_one(&self.pool)
        .await
        .map(|n| n > 0)
        .unwrap_or(false)
    }

    /// 채널이 하나도 없으면 테스트 채널을 넣는다.
    async fn seed_if_empty(&self) -> Result<(), String> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM channels")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        if count == 0 {
            for (name, key) in SEED_CHANNEL_NAMES.iter().zip(SEED_STREAM_KEYS) {
                sqlx::query("INSERT INTO channels (name, stream_key) VALUES (?, ?)")
                    .bind(name)
                    .bind(key)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().await.map_err(|e| e.to_string())
    }
}
