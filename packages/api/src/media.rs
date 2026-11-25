//! MediaMTX와 말하는 부분. 서비스는 `MediaServer`만 알아서 MediaMTX 없이도 테스트할 수 있다.
use std::sync::Arc;

/// 서비스가 영상 서버에 시키는 일. 
#[async_trait::async_trait]
pub trait MediaServer: Send + Sync {
    /// 채널 경로(`live/<채널 아이디>`)로 송출 중인 연결을 끊는다. 송출 중이 아니면 아무 일도 없다.
    async fn kick_publisher(&self, channel_id: i64) -> Result<(), String>;

    /// 채널 경로로 영상 신호가 지금 들어오고 있는지. 경로가 없으면 `false`.
    async fn has_signal(&self, channel_id: i64) -> Result<bool, String>;
}

/// 아무것도 하지 않는 구현. 영상 서버 없이 서비스만 쓸 때(테스트 기본값).
pub struct NoMediaServer;

#[async_trait::async_trait]
impl MediaServer for NoMediaServer {
    async fn kick_publisher(&self, _: i64) -> Result<(), String> {
        Ok(())
    }
    async fn has_signal(&self, _: i64) -> Result<bool, String> {
        Ok(false)
    }
}

/// MediaMTX Control API(`apiAddress`, 내 PC에서만 열린다)를 부르는 구현.
pub struct MediaMtxClient {
    api_base: String,
    http: reqwest::Client,
}

impl MediaMtxClient {
    /// 환경 변수 `ONAIR_MEDIAMTX_API`, 없으면 `http://127.0.0.1:9997`.
    pub fn from_env() -> Arc<Self> {
        let base = std::env::var("ONAIR_MEDIAMTX_API")
            .unwrap_or_else(|_| "http://127.0.0.1:9997".to_string());
        Arc::new(Self::new(base))
    }

    pub fn new(api_base: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .expect("HTTP 클라이언트 생성");
        Self {
            api_base: api_base.into().trim_end_matches('/').to_string(),
            http,
        }
    }
}

impl MediaMtxClient {
    /// `live/<채널 아이디>` 경로의 현재 상태(JSON). 경로가 없으면 `None`.
    async fn fetch_path(&self, channel_id: i64) -> Result<Option<serde_json::Value>, String> {
        let url = format!("{}/v3/paths/get/live/{channel_id}", self.api_base);
        let resp = self.http.get(url).send().await.map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let path = resp
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Some(path))
    }
}

#[async_trait::async_trait]
impl MediaServer for MediaMtxClient {
    async fn kick_publisher(&self, channel_id: i64) -> Result<(), String> {
        // 경로가 없으면 송출 중인 연결도 없다.
        let Some(path) = self.fetch_path(channel_id).await? else {
            return Ok(());
        };
        let source = &path["source"];
        if source["type"] != "rtmpConn" {
            return Ok(());
        }
        let Some(conn_id) = source["id"].as_str() else {
            return Ok(());
        };
        // v1.21.4에서 `/v3/rtmp/conns/kick`과 예전 `/v3/rtmpconns/kick` 둘 다 동작함을 확인했다.
        let url = format!("{}/v3/rtmp/conns/kick/{conn_id}", self.api_base);
        let resp = self.http.post(url).send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("송출 끊기(kick) 요청이 HTTP {status}로 거부됨"));
        }
        Ok(())
    }

    async fn has_signal(&self, channel_id: i64) -> Result<bool, String> {
        let path = self.fetch_path(channel_id).await?;
        Ok(path.is_some_and(|p| p["online"].as_bool().unwrap_or(false)))
    }
}
