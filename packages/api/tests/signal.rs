//! 영상 신호 상태 테스트. MediaMTX 없이 신호 여부를 바꿀 수 있는 가짜 MediaServer를 끼운다.
#![cfg(feature = "server")]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use api::service::{MediaServer, OnairService, SEED_STREAM_KEYS};

/// 신호가 있는 채널을 테스트가 정하는 가짜.
#[derive(Default)]
struct FakeMedia {
    signal: Mutex<HashSet<i64>>,
}

#[async_trait::async_trait]
impl MediaServer for FakeMedia {
    async fn kick_publisher(&self, _: i64) -> Result<(), String> {
        Ok(())
    }
    async fn has_signal(&self, channel_id: i64) -> Result<bool, String> {
        Ok(self.signal.lock().unwrap().contains(&channel_id))
    }
}

async fn open(dir: &std::path::Path) -> (OnairService, Arc<FakeMedia>) {
    let media = Arc::new(FakeMedia::default());
    let service = OnairService::open_with_media(dir.join("onair.sqlite"), dir.join("uploads"), media.clone())
        .await
        .unwrap();
    (service, media)
}

#[tokio::test]
async fn 방송_중에_신호가_들어오면_신호_있음이다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, media) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();
    media.signal.lock().unwrap().insert(id);

    let status = service.channel_status(id).await.unwrap();

    assert_eq!(status.live_title.as_deref(), Some("방송"));
    assert!(status.has_signal);
}

#[tokio::test]
async fn 방송_중인데_신호가_없으면_신호_없음이다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();

    let status = service.channel_status(id).await.unwrap();

    assert_eq!(status.live_title.as_deref(), Some("방송"));
    assert!(!status.has_signal);
}

#[tokio::test]
async fn 신호가_끊겨도_방송은_자동으로_종료되지_않고_돌아오면_다시_있음이다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, media) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();
    media.signal.lock().unwrap().insert(id);
    assert!(service.channel_status(id).await.unwrap().has_signal);

    media.signal.lock().unwrap().remove(&id);
    let lost = service.channel_status(id).await.unwrap();
    assert!(!lost.has_signal);
    assert_eq!(lost.live_title.as_deref(), Some("방송"));
    assert_eq!(service.get_channel(id).await.unwrap().live_title.as_deref(), Some("방송"));

    media.signal.lock().unwrap().insert(id);
    assert!(service.channel_status(id).await.unwrap().has_signal);
}

#[tokio::test]
async fn 방송_중이_아니면_신호가_있어도_신호_없음이다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, media) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    media.signal.lock().unwrap().insert(id);

    let status = service.channel_status(id).await.unwrap();

    assert_eq!(status.live_title, None);
    assert!(!status.has_signal);
}

#[tokio::test]
async fn 방송을_종료하면_상태가_방송_중_아님으로_바뀐다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, media) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();
    media.signal.lock().unwrap().insert(id);
    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();

    let status = service.channel_status(id).await.unwrap();

    assert_eq!(status.live_title, None);
    assert!(!status.has_signal);
}

#[tokio::test]
async fn 없는_채널은_이유와_함께_실패한다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;

    assert_eq!(service.channel_status(9999).await.unwrap_err(), "없는 채널입니다");
}

struct BrokenMedia;

#[async_trait::async_trait]
impl MediaServer for BrokenMedia {
    async fn kick_publisher(&self, _: i64) -> Result<(), String> {
        Ok(())
    }
    async fn has_signal(&self, _: i64) -> Result<bool, String> {
        Err("MediaMTX가 꺼져 있다".to_string())
    }
}

#[tokio::test]
async fn 영상_서버에_물을_수_없으면_신호_없음으로_본다() {
    let dir = tempfile::tempdir().unwrap();
    let service = OnairService::open_with_media(
        dir.path().join("onair.sqlite"),
        dir.path().join("uploads"),
        Arc::new(BrokenMedia),
    )
    .await
    .unwrap();
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();

    let status = service.channel_status(id).await.unwrap();

    assert_eq!(status.live_title.as_deref(), Some("방송"));
    assert!(!status.has_signal);
}
