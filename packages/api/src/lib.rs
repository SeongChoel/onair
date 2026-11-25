//! onair 서버 쪽 전부. 규칙은 `service::OnairService` 한 곳에 모은다.
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "server")]
pub mod service;

/// 채널 목록의 한 항목. 시청자에게 나가므로 송출 키는 담지 않는다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelSummary {
    pub id: i64,
    pub name: String,
}

/// 채널 목록. onair 서비스를 그대로 부르는 얇은 통로.
#[get("/api/channels")]
pub async fn list_channels() -> Result<Vec<ChannelSummary>, ServerFnError> {
    server::service()
        .list_channels()
        .await
        .map_err(ServerFnError::new)
}
