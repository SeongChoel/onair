//! 채팅 도배 방지 테스트. 같은 채널·같은 닉네임으로 5초 안에 5개까지만 보낼 수 있다.
#![cfg(feature = "server")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use api::service::{OnairService, SEED_STREAM_KEYS};

const FLOOD: &str = "잠시 후 다시 보내세요";

/// 테스트에서 시간을 직접 움직이는 시계.
#[derive(Clone)]
struct FakeClock(Arc<Mutex<Instant>>);

impl FakeClock {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Instant::now())))
    }
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}

/// 시드 채널 1·2에 방송을 켠 서비스와 두 채널 아이디.
async fn live_service(dir: &std::path::Path, clock: &FakeClock) -> (OnairService, i64, i64) {
    let c = clock.clone();
    let service = OnairService::open(dir.join("onair.sqlite"), dir.join("uploads"))
        .await
        .unwrap()
        .with_clock(move || *c.0.lock().unwrap());
    let channels = service.list_channels().await.unwrap();
    service.start_broadcast(SEED_STREAM_KEYS[0], "").await.unwrap();
    service.start_broadcast(SEED_STREAM_KEYS[1], "").await.unwrap();
    (service, channels[0].id, channels[1].id)
}

async fn send_five(service: &OnairService, channel: i64, nick: &str) {
    for i in 0..5 {
        service.send_chat(channel, nick, &format!("{i}")).await.unwrap();
    }
}

#[tokio::test]
async fn 같은_닉네임으로_5초_안에_6번째_메시지는_거부되고_저장되지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::new();
    let (service, channel, _) = live_service(dir.path(), &clock).await;
    send_five(&service, channel, "철수").await;

    clock.advance(Duration::from_millis(4_900));
    let sixth = service.send_chat(channel, "철수", "도배").await;

    assert_eq!(sixth.unwrap_err(), FLOOD);
    assert_eq!(service.recent_chat(channel).await.unwrap().len(), 5);
}

#[tokio::test]
async fn 첫_메시지로부터_5초가_지나면_다시_보낼_수_있다() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::new();
    let (service, channel, _) = live_service(dir.path(), &clock).await;
    send_five(&service, channel, "철수").await;

    clock.advance(Duration::from_secs(5));

    service.send_chat(channel, "철수", "다시").await.unwrap();
}

#[tokio::test]
async fn 다른_닉네임과_다른_채널은_한도가_따로다() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::new();
    let (service, one, two) = live_service(dir.path(), &clock).await;
    send_five(&service, one, "철수").await;

    service.send_chat(one, "영희", "안녕").await.unwrap();
    service.send_chat(two, "철수", "안녕").await.unwrap();
}

#[tokio::test]
async fn 거부된_메시지는_한도_계산에_들어가지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::new();
    let (service, channel, _) = live_service(dir.path(), &clock).await;
    // 0초에 1개, 3초에 4개를 보낸 뒤 4초에 거부된다.
    service.send_chat(channel, "철수", "처음").await.unwrap();
    clock.advance(Duration::from_secs(3));
    for i in 0..4 {
        service.send_chat(channel, "철수", &format!("{i}")).await.unwrap();
    }
    clock.advance(Duration::from_secs(1));
    assert_eq!(service.send_chat(channel, "철수", "거부").await.unwrap_err(), FLOOD);

    // 5초가 되면 처음 메시지가 빠져 한 자리가 생긴다. 거부된 시도가 셌다면 여전히 막힌다.
    clock.advance(Duration::from_secs(1));
    service.send_chat(channel, "철수", "통과").await.unwrap();
}

#[tokio::test]
async fn 닉네임_앞뒤_공백은_같은_닉네임으로_센다() {
    let dir = tempfile::tempdir().unwrap();
    let clock = FakeClock::new();
    let (service, channel, _) = live_service(dir.path(), &clock).await;
    send_five(&service, channel, "철수").await;

    assert_eq!(
        service.send_chat(channel, " 철수 ", "우회").await.unwrap_err(),
        FLOOD
    );
}
