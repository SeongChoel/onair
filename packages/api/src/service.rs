//! onair 서비스: 모든 규칙을 가진다. 서버 함수와 HTTP 핸들러는 이것을 부르는 얇은 통로다.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

pub use crate::media::{MediaMtxClient, MediaServer, NoMediaServer};
use crate::{ChannelStatus, ChannelSummary, ChatMessage, StudioChannel};

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

/// 송출 키: 고정 접두어 + 무작위 영숫자 32자. RTMP 주소의 쿼리에 그대로 실린다.
fn new_stream_key() -> String {
    use rand::distr::{Alphanumeric, SampleString};
    format!(
        "onair_{}",
        Alphanumeric.sample_string(&mut rand::rng(), 32)
    )
}

const MAX_NICKNAME_CHARS: usize = 20;
const MAX_CHAT_CHARS: usize = 500;
const RECENT_CHAT_LIMIT: i64 = 50;

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

fn random_suffix() -> String {
    use rand::distr::{Alphanumeric, SampleString};
    Alphanumeric.sample_string(&mut rand::rng(), 12)
}

pub struct OnairService {
    pool: SqlitePool,
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
            upload_dir,
            media,
        };
        service.seed_if_empty().await?;
        Ok(service)
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
        let live_title = self.get_channel(id).await?.live_title;
        let has_signal = match live_title {
            Some(_) => self.media.has_signal(id).await.unwrap_or_else(|e| {
                eprintln!("영상 신호 확인 실패(채널 {id}): {e}");
                false
            }),
            None => false,
        };
        Ok(ChannelStatus {
            live_title,
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
        .ok_or_else(|| "송출 키가 올바르지 않습니다".to_string())
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
            .ok_or_else(|| "송출 키가 올바르지 않습니다".to_string())?;
        let id: i64 = row.get("id");
        let old: Option<String> = row.get("profile_image");

        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("이미지는 2MB 이하여야 합니다".to_string());
        }
        let ext = image_extension(bytes)
            .ok_or_else(|| "jpg, png, webp 이미지만 올릴 수 있습니다".to_string())?;

        // 파일 이름에 송출 키를 쓰지 않는다. 이름이 바뀌어야 브라우저 캐시도 새로 받는다.
        let file_name = format!("channel-{id}-{}.{ext}", random_suffix());
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
        let done = sqlx::query(
            "UPDATE broadcasts SET ended_at = CURRENT_TIMESTAMP WHERE channel_id = ? AND ended_at IS NULL",
        )
        .bind(channel.id)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        if done.rows_affected() == 0 {
            return Err("진행 중인 방송이 없습니다".to_string());
        }
        // 이미 연결된 송출도 끊는다. 실패해도 종료는 유효하다(OBS가 다시 붙으면 허락 확인에서 거부된다).
        if let Err(e) = self.media.kick_publisher(channel.id).await {
            eprintln!("송출 끊기 실패(채널 {}): {e}", channel.id);
        }
        Ok(())
    }

    /// 채팅 보내기. 채널의 진행 중인 방송에 저장한다. 닉네임은 앞뒤 공백을 자르며 1~20자,
    /// 내용은 비어 있지 않고 500자 이하여야 한다. 진행 중인 방송이 없으면 거부한다.
    /// (이후 구독·전달은 저장 직후 이 함수에서 보낸다. 시그니처는 그대로 둔다.)
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
        let row = sqlx::query(
            "INSERT INTO chat_messages (broadcast_id, nickname, content)
             SELECT b.id, ?, ? FROM broadcasts b WHERE b.channel_id = ? AND b.ended_at IS NULL
             RETURNING id, nickname, content, sent_at",
        )
        .bind(nickname)
        .bind(content)
        .bind(channel_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "방송 중이 아니라 채팅할 수 없습니다".to_string())?;
        Ok(to_chat_message(&row))
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
