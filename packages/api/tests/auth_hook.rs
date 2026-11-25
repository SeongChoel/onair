//! MediaMTX 인증 훅 서버 테스트. 실제로 소켓에 붙어 HTTP로 묻는다.
#![cfg(feature = "server")]

use api::service::SEED_STREAM_KEYS;

async fn ask(base: &str, body: &str) -> reqwest::StatusCode {
    reqwest::Client::new()
        .post(format!("{base}/internal/mediamtx/auth"))
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn 인증_훅은_내_pc_주소에만_열리고_송출_허락을_판단한다() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("ONAIR_DATA_DIR", dir.path());
    let service = api::server::init().await.unwrap();
    let id = service.list_channels().await.unwrap()[0].id;

    // 포트 0: 빈 포트를 받는다. 어느 포트든 루프백에만 열려야 한다.
    let addr = api::server::serve_mediamtx_auth(0).await.unwrap();
    assert!(addr.ip().is_loopback(), "{addr}");
    let base = format!("http://{addr}");

    let publish = format!(
        r#"{{"action":"publish","path":"live/{id}","query":"key={}"}}"#,
        SEED_STREAM_KEYS[0]
    );
    // 방송 전에는 맞는 키도 거부한다.
    assert_eq!(ask(&base, &publish).await, reqwest::StatusCode::FORBIDDEN);
    service.start_broadcast(SEED_STREAM_KEYS[0], "").await.unwrap();
    assert_eq!(ask(&base, &publish).await, reqwest::StatusCode::OK);
    service.close().await;
}
