//! 채팅 테스트. 서비스(`OnairService`)의 채팅 보내기·최근 채팅만 본다.
#![cfg(feature = "server")]

use api::service::{OnairService, SEED_STREAM_KEYS};

async fn open(dir: &std::path::Path) -> OnairService {
    OnairService::open(dir.join("onair.sqlite"), dir.join("uploads"))
        .await
        .unwrap()
}

/// 시드 채널 1의 아이디.
async fn first_channel(service: &OnairService) -> i64 {
    service.list_channels().await.unwrap()[0].id
}

#[tokio::test]
async fn 라이브_중인_채널에_보낸_메시지가_닉네임과_함께_최근_채팅에_보인다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = first_channel(&service).await;
    service
        .start_broadcast(SEED_STREAM_KEYS[0], "")
        .await
        .unwrap();

    let sent = service.send_chat(channel, "철수", "안녕하세요").await.unwrap();

    let recent = service.recent_chat(channel).await.unwrap();
    assert_eq!(recent, [sent.clone()]);
    assert_eq!(sent.nickname, "철수");
    assert_eq!(sent.content, "안녕하세요");
}

async fn live(service: &OnairService) -> i64 {
    let channel = first_channel(service).await;
    service
        .start_broadcast(SEED_STREAM_KEYS[0], "")
        .await
        .unwrap();
    channel
}

#[tokio::test]
async fn 같은_닉네임을_여러_번_써도_된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = live(&service).await;

    service.send_chat(channel, "철수", "하나").await.unwrap();
    service.send_chat(channel, "철수", "둘").await.unwrap();

    assert_eq!(service.recent_chat(channel).await.unwrap().len(), 2);
}

#[tokio::test]
async fn 닉네임을_바꿔도_이미_보낸_메시지의_닉네임은_그대로다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = live(&service).await;

    service.send_chat(channel, "철수", "하나").await.unwrap();
    service.send_chat(channel, "영희", "둘").await.unwrap();

    let nicknames: Vec<String> = service
        .recent_chat(channel)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.nickname)
        .collect();
    assert_eq!(nicknames, ["철수", "영희"]);
}

#[tokio::test]
async fn 잘못된_메시지와_닉네임은_이유와_함께_거부되고_저장되지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = live(&service).await;

    let long_content = "가".repeat(501);
    let long_nick = "가".repeat(21);
    let cases = [
        ("철수", "", "메시지를 입력하세요"),
        ("철수", "   ", "메시지를 입력하세요"),
        ("철수", long_content.as_str(), "메시지는 500자 이하여야 합니다"),
        ("", "안녕", "닉네임을 입력하세요"),
        ("  ", "안녕", "닉네임을 입력하세요"),
        (long_nick.as_str(), "안녕", "닉네임은 20자 이하여야 합니다"),
    ];
    for (nick, content, reason) in cases {
        assert_eq!(
            service.send_chat(channel, nick, content).await.unwrap_err(),
            reason
        );
    }
    assert!(service.recent_chat(channel).await.unwrap().is_empty());
}

#[tokio::test]
async fn 경계_길이는_허용된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = live(&service).await;

    service
        .send_chat(channel, &"나".repeat(20), &"가".repeat(500))
        .await
        .unwrap();

    assert_eq!(service.recent_chat(channel).await.unwrap().len(), 1);
}

#[tokio::test]
async fn 최근_채팅은_마지막_50개를_오래된_순으로_돌려준다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = live(&service).await;
    for i in 1..=60 {
        service
            .send_chat(channel, &format!("시청자{i}"), &format!("메시지 {i}"))
            .await
            .unwrap();
    }

    let recent = service.recent_chat(channel).await.unwrap();

    assert_eq!(recent.len(), 50);
    assert_eq!(recent.first().unwrap().content, "메시지 11");
    assert_eq!(recent.last().unwrap().content, "메시지 60");
}

#[tokio::test]
async fn 라이브_중이_아닌_채널에는_보낼_수_없다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = first_channel(&service).await;

    let before = service.send_chat(channel, "철수", "안녕").await;
    assert_eq!(before.unwrap_err(), "방송 중이 아니라 채팅할 수 없습니다");

    service.start_broadcast(SEED_STREAM_KEYS[0], "").await.unwrap();
    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();
    let after = service.send_chat(channel, "철수", "안녕").await;
    assert_eq!(after.unwrap_err(), "방송 중이 아니라 채팅할 수 없습니다");
    assert!(service.recent_chat(channel).await.unwrap().is_empty());
}

#[tokio::test]
async fn 다른_채널_방송의_채팅과_섞이지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channels = service.list_channels().await.unwrap();
    service.start_broadcast(SEED_STREAM_KEYS[0], "").await.unwrap();
    service.start_broadcast(SEED_STREAM_KEYS[1], "").await.unwrap();

    service.send_chat(channels[0].id, "철수", "첫째 채널").await.unwrap();

    assert!(service.recent_chat(channels[1].id).await.unwrap().is_empty());
}

#[tokio::test]
async fn 새_방송의_채팅은_비어_있고_끝난_방송의_메시지는_지워지지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = live(&service).await;
    service.send_chat(channel, "철수", "첫 방송").await.unwrap();
    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();

    service.start_broadcast(SEED_STREAM_KEYS[0], "").await.unwrap();
    assert!(service.recent_chat(channel).await.unwrap().is_empty());
    service.send_chat(channel, "영희", "새 방송").await.unwrap();
    let contents: Vec<String> = service
        .recent_chat(channel)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.content)
        .collect();
    assert_eq!(contents, ["새 방송"]);

    // 끝난 방송의 메시지는 DB에 그대로 남아 있다(서비스 밖에서 직접 센다).
    service.close().await;
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(dir.path().join("onair.sqlite"));
    let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 2);
}
