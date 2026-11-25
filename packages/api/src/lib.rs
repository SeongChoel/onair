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
    pub has_profile_image: bool,
}

/// 채널 목록. onair 서비스를 그대로 부르는 얇은 통로.
#[get("/api/channels")]
pub async fn list_channels() -> Result<Vec<ChannelSummary>, ServerFnError> {
    server::service()
        .list_channels()
        .await
        .map_err(ServerFnError::new)
}

/// 채널 하나. 채널 페이지가 채널 아이디로 부른다.
#[get("/api/channels/{id}")]
pub async fn get_channel(id: i64) -> Result<ChannelSummary, ServerFnError> {
    server::service()
        .get_channel(id)
        .await
        .map_err(ServerFnError::new)
}
