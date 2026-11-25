//! onair 서비스 테스트. 테스트마다 비어 있는 임시 SQLite 파일과 임시 업로드 폴더로 서비스를 만든다.
#![cfg(feature = "server")]

use api::service::OnairService;

#[tokio::test]
async fn 새_서비스에는_채널이_없다() {
    let dir = tempfile::tempdir().unwrap();
    let service = OnairService::open(dir.path().join("onair.sqlite"), dir.path().join("uploads"))
        .await
        .unwrap();

    let channels = service.list_channels().await.unwrap();

    assert!(channels.is_empty());
}
