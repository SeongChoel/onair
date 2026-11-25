//! onair 서비스: 모든 규칙을 가진다. 서버 함수와 HTTP 핸들러는 이것을 부르는 얇은 통로다.
use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::{ChannelSummary, StudioChannel};

const SCHEMA: &str = include_str!("schema.sql");

/// 테스트 채널 이름과 송출 키. 키는 고정값이며 docs/dev.md에 적혀 있다.
const SEED_CHANNEL_NAMES: [&str; 3] = ["테스트 채널 1", "테스트 채널 2", "테스트 채널 3"];
pub const SEED_STREAM_KEYS: [&str; 3] = ["test-key-1", "test-key-2", "test-key-3"];

fn to_summary(r: &sqlx::sqlite::SqliteRow) -> ChannelSummary {
    let image: Option<String> = r.get("profile_image");
    ChannelSummary {
        id: r.get("id"),
        name: r.get("name"),
        has_profile_image: image.is_some(),
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

pub struct OnairService {
    pool: SqlitePool,
    upload_dir: PathBuf,
}

impl OnairService {
    /// SQLite 파일과 업로드 폴더 위치를 받아 서비스를 만든다. 없으면 만든다.
    pub async fn open(
        db_path: impl AsRef<Path>,
        upload_dir: impl Into<PathBuf>,
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
        let service = Self { pool, upload_dir };
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
        let rows = sqlx::query("SELECT id, name, profile_image FROM channels ORDER BY id")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(to_summary).collect())
    }

    /// 채널 하나. 없으면 사용자에게 보여 줄 수 있는 이유로 실패한다.
    pub async fn get_channel(&self, id: i64) -> Result<ChannelSummary, String> {
        let row = sqlx::query("SELECT id, name, profile_image FROM channels WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        row.as_ref()
            .map(to_summary)
            .ok_or_else(|| "없는 채널입니다".to_string())
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
            }),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                Err("이미 있는 채널 이름입니다".to_string())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// 스튜디오 들어가기. 송출 키가 없으면 실패한다.
    pub async fn enter_studio(&self, stream_key: &str) -> Result<StudioChannel, String> {
        let row = sqlx::query("SELECT id, name, stream_key FROM channels WHERE stream_key = ?")
            .bind(stream_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        row.map(|r| StudioChannel {
            id: r.get("id"),
            name: r.get("name"),
            stream_key: r.get("stream_key"),
        })
        .ok_or_else(|| "송출 키가 올바르지 않습니다".to_string())
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
