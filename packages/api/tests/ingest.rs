//! 영상 수신 허락과 방송 종료 시 송출 끊기 테스트. MediaMTX 없이 가짜 MediaServer를 끼운다.
#![cfg(feature = "server")]

use std::sync::{Arc, Mutex};

use api::service::{MediaServer, OnairService, SEED_STREAM_KEYS};

/// 끊기 요청을 기록만 하는 가짜.
#[derive(Default)]
struct FakeMedia {
    kicked: Mutex<Vec<i64>>,
}

#[async_trait::async_trait]
impl MediaServer for FakeMedia {
    async fn kick_publisher(&self, channel_id: i64) -> Result<(), String> {
        self.kicked.lock().unwrap().push(channel_id);
        Ok(())
    }
    async fn has_signal(&self, _: i64) -> Result<bool, String> {
        Ok(false)
    }
}

async fn open(dir: &std::path::Path) -> (OnairService, Arc<FakeMedia>) {
    let media = Arc::new(FakeMedia::default());
    let service = OnairService::open_with_media(
        dir.join("onair.sqlite"),
        dir.join("uploads"),
        media.clone(),
    )
    .await
    .unwrap();
    (service, media)
}

#[tokio::test]
async fn 방송_시작_전에는_송출이_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;

    let ok = service
        .authorize_ingest("publish", &format!("live/{id}"), &format!("key={}", SEED_STREAM_KEYS[0]))
        .await;

    assert!(!ok);
}

#[tokio::test]
async fn 진행_중인_방송에_맞는_키로_송출하면_허락된다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();

    let ok = service
        .authorize_ingest("publish", &format!("live/{id}"), &format!("key={}", SEED_STREAM_KEYS[0]))
        .await;

    assert!(ok);
}

#[tokio::test]
async fn 틀린_키나_다른_채널의_키는_송출이_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();
    let path = format!("live/{id}");

    assert!(!service.authorize_ingest("publish", &path, "key=wrong").await);
    assert!(!service.authorize_ingest("publish", &path, "").await);
    // 다른 채널의 키 (그 채널도 방송 중이어도 이 경로에는 쓸 수 없다)
    service.start_broadcast(SEED_STREAM_KEYS[1], "방송2").await.unwrap();
    let other = format!("key={}", SEED_STREAM_KEYS[1]);
    assert!(!service.authorize_ingest("publish", &path, &other).await);
}

#[tokio::test]
async fn 방송_종료_후에는_송출이_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();
    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();

    let ok = service
        .authorize_ingest("publish", &format!("live/{id}"), &format!("key={}", SEED_STREAM_KEYS[0]))
        .await;

    assert!(!ok);
}

#[tokio::test]
async fn 경로_형식이_틀리거나_없는_채널이면_송출이_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();
    let q = format!("key={}", SEED_STREAM_KEYS[0]);

    for path in ["", "live", "live/", "live/abc", "other/1", "live/1/extra", "live/9999", "test-key-1"] {
        assert!(!service.authorize_ingest("publish", path, &q).await, "{path}");
    }
}

#[tokio::test]
async fn 쿼리에_다른_값이_섞여도_key를_찾는다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();

    let q = format!("foo=bar&key={}", SEED_STREAM_KEYS[0]);

    assert!(service.authorize_ingest("publish", &format!("live/{id}"), &q).await);
}

#[tokio::test]
async fn 재생은_항상_허락되고_알_수_없는_동작은_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, _) = open(dir.path()).await;

    assert!(service.authorize_ingest("read", "live/1", "").await);
    assert!(service.authorize_ingest("playback", "live/1", "").await);
    assert!(!service.authorize_ingest("api", "", "").await);
    assert!(!service.authorize_ingest("whatever", "live/1", "").await);
}

#[tokio::test]
async fn 방송을_종료하면_연결된_송출이_끊긴다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, media) = open(dir.path()).await;
    let id = service.list_channels().await.unwrap()[0].id;
    service.start_broadcast(SEED_STREAM_KEYS[0], "방송").await.unwrap();
    assert!(media.kicked.lock().unwrap().is_empty());

    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();

    assert_eq!(*media.kicked.lock().unwrap(), vec![id]);
}

#[tokio::test]
async fn 종료가_거부되면_송출을_끊지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let (service, media) = open(dir.path()).await;

    assert!(service.end_broadcast(SEED_STREAM_KEYS[0]).await.is_err());

    assert!(media.kicked.lock().unwrap().is_empty());
}

struct BrokenMedia;

#[async_trait::async_trait]
impl MediaServer for BrokenMedia {
    async fn kick_publisher(&self, _: i64) -> Result<(), String> {
        Err("MediaMTX가 꺼져 있다".to_string())
    }
    async fn has_signal(&self, _: i64) -> Result<bool, String> {
        Err("MediaMTX가 꺼져 있다".to_string())
    }
}

#[tokio::test]
async fn 끊기에_실패해도_방송_종료는_성공한다() {
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

    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();

    assert_eq!(service.get_channel(id).await.unwrap().live_title, None);
}
