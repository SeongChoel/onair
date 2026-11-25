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

/// 스튜디오에 들어간 스트리머에게만 나가는 채널. 송출 키를 담는다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudioChannel {
    pub id: i64,
    pub name: String,
    pub stream_key: String,
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

/// 채널 만들기. 스튜디오에서 이름을 받아 채널과 송출 키를 돌려준다.
#[post("/api/studio/channels")]
pub async fn create_channel(name: String) -> Result<StudioChannel, ServerFnError> {
    server::service()
        .create_channel(&name)
        .await
        .map_err(ServerFnError::new)
}

/// 스튜디오 들어가기. 송출 키가 맞아야 채널을 돌려준다.
#[post("/api/studio/enter")]
pub async fn enter_studio(stream_key: String) -> Result<StudioChannel, ServerFnError> {
    server::service()
        .enter_studio(&stream_key)
        .await
        .map_err(ServerFnError::new)
}
