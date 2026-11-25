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
    /// 프로필 이미지를 가져오는 주소(`/uploads/...`). 없으면 기본 이미지를 보여 준다.
    pub profile_image: Option<String>,
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

/// 프로필 이미지 올리기. 멀티파트 폼의 `key`(송출 키)와 `file`(이미지) 필드를 받는다.
/// 본문 한도는 라우터의 `DefaultBodyLimit`(4MB)이고, 2MB 판정은 서비스가 한다.
#[post("/api/studio/profile-image")]
pub async fn upload_profile_image(
    mut form: dioxus::fullstack::MultipartFormData,
) -> Result<ChannelSummary, ServerFnError> {
    let (mut key, mut file) = (None, None);
    // 본문 한도를 넘으면 읽기가 실패한다. 이유를 알 수 있게 크기 문구로 돌려준다.
    let too_big = || ServerFnError::new("이미지는 2MB 이하여야 합니다");
    while let Some(field) = form.next_field().await.map_err(|_| too_big())? {
        match field.name() {
            Some("key") => key = Some(field.text().await.map_err(|_| too_big())?),
            Some("file") => file = Some(field.bytes().await.map_err(|_| too_big())?),
            _ => {}
        }
    }
    let (Some(key), Some(file)) = (key, file) else {
        return Err(ServerFnError::new("송출 키와 이미지 파일이 필요합니다"));
    };
    server::service()
        .upload_profile_image(&key, &file)
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
