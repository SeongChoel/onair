//! onair 서비스: 모든 규칙을 가진다. 서버 함수와 HTTP 핸들러는 이것을 부르는 얇은 통로다.
use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::ChannelSummary;

const SCHEMA: &str = include_str!("schema.sql");

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
        Ok(Self { pool, upload_dir })
    }

    pub fn upload_dir(&self) -> &Path {
        &self.upload_dir
    }

    pub async fn list_channels(&self) -> Result<Vec<ChannelSummary>, String> {
        let rows = sqlx::query("SELECT id, name FROM channels ORDER BY id")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows
            .iter()
            .map(|r| ChannelSummary {
                id: r.get("id"),
                name: r.get("name"),
            })
            .collect())
    }
}
