//! 실행 중인 서버가 쓰는 전역 onair 서비스. 테스트는 이것을 쓰지 않고 서비스를 직접 만든다.
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::OnceCell;

use crate::service::OnairService;

static SERVICE: OnceCell<Arc<OnairService>> = OnceCell::const_new();

/// 데이터 폴더: 환경 변수 `ONAIR_DATA_DIR`, 없으면 `%LOCALAPPDATA%\onair\data`.
/// 저장소가 OneDrive 안에 있어서 기본값을 그 밖으로 잡았다.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ONAIR_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("onair").join("data")
}

/// 서비스를 한 번만 만든다. 핫리로드로 서버 시작 콜백이 다시 불려도 안전하다.
pub async fn init() -> Result<Arc<OnairService>, String> {
    SERVICE
        .get_or_try_init(|| async {
            let dir = data_dir();
            let service = OnairService::open_with_media(
                dir.join("onair.sqlite"),
                dir.join("uploads"),
                crate::media::MediaMtxClient::from_env(),
            )
            .await?;
            Ok(Arc::new(service))
        })
        .await
        .cloned()
}

/// MediaMTX가 보내는 확인 요청(JSON). 쓰는 필드만 읽는다.
#[derive(serde::Deserialize)]
struct MediaAuthRequest {
    #[serde(default)]
    action: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    query: String,
}

/// MediaMTX `authHTTPAddress`가 부르는 경로. 판단은 서비스가 하고, 여기서는 상태 코드로만 바꾼다.
/// 20x면 허락, 403이면 거부다.
async fn mediamtx_auth(
    dioxus::server::axum::Json(req): dioxus::server::axum::Json<MediaAuthRequest>,
) -> dioxus::server::axum::http::StatusCode {
    use dioxus::server::axum::http::StatusCode;
    if service()
        .authorize_ingest(&req.action, &req.path, &req.query)
        .await
    {
        StatusCode::OK
    } else {
        StatusCode::FORBIDDEN
    }
}

/// `POST /internal/mediamtx/auth` 하나짜리 라우터. 앱 라우터에 `merge`한다.
pub fn mediamtx_auth_router() -> dioxus::server::axum::Router {
    dioxus::server::axum::Router::new().route(
        "/internal/mediamtx/auth",
        dioxus::server::axum::routing::post(mediamtx_auth),
    )
}

pub fn service() -> Arc<OnairService> {
    SERVICE
        .get()
        .cloned()
        .expect("서버 시작 시 api::server::init()을 먼저 불러야 한다")
}
