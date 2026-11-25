//! 실시간 채팅 전달 테스트. 서비스(`OnairService`)의 구독만 본다. WebSocket은 거치지 않는다.
#![cfg(feature = "server")]

use std::time::Duration;

use api::service::{ChatSubscription, OnairService, SEED_STREAM_KEYS};
use api::ChatServerMsg;

async fn open(dir: &std::path::Path) -> OnairService {
    OnairService::open(dir.join("onair.sqlite"), dir.join("uploads"))
        .await
        .unwrap()
}

/// 시드 채널 `n`(0부터)의 방송을 시작하고 그 채널 아이디를 돌려준다.
async fn go_live(service: &OnairService, n: usize) -> i64 {
    let id = service.list_channels().await.unwrap()[n].id;
    service
        .start_broadcast(SEED_STREAM_KEYS[n], "")
        .await
        .unwrap();
    id
}

/// 잠시 기다려도 아무것도 오지 않으면 true.
async fn nothing_arrives(sub: &mut ChatSubscription) -> bool {
    tokio::time::timeout(Duration::from_millis(100), sub.recv())
        .await
        .is_err()
}

#[tokio::test]
async fn 한쪽이_보내면_같은_방송을_구독한_다른_쪽이_받는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = go_live(&service, 0).await;
    let mut a = service.subscribe_chat(channel).await.unwrap();
    let mut b = service.subscribe_chat(channel).await.unwrap();

    let sent = service.send_chat(channel, "철수", "안녕").await.unwrap();

    assert_eq!(a.recv().await, Some(ChatServerMsg::Message(sent.clone())));
    assert_eq!(b.recv().await, Some(ChatServerMsg::Message(sent)));
}

#[tokio::test]
async fn 다른_채널의_메시지는_받지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let one = go_live(&service, 0).await;
    let two = go_live(&service, 1).await;
    let mut sub = service.subscribe_chat(one).await.unwrap();

    service.send_chat(two, "영희", "다른 방").await.unwrap();

    assert!(nothing_arrives(&mut sub).await);
}

#[tokio::test]
async fn 방송이_바뀌면_이전_방송_구독에_새_방송_메시지가_오지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = go_live(&service, 0).await;
    let mut old = service.subscribe_chat(channel).await.unwrap();
    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();
    assert_eq!(old.recv().await, Some(ChatServerMsg::Ended));
    service
        .start_broadcast(SEED_STREAM_KEYS[0], "")
        .await
        .unwrap();

    service.send_chat(channel, "철수", "새 방송").await.unwrap();

    assert_eq!(old.recv().await, None);
}

#[tokio::test]
async fn 방송이_종료되면_구독에_종료가_전달된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = go_live(&service, 0).await;
    let mut sub = service.subscribe_chat(channel).await.unwrap();

    service.end_broadcast(SEED_STREAM_KEYS[0]).await.unwrap();

    assert_eq!(sub.recv().await, Some(ChatServerMsg::Ended));
    assert_eq!(sub.recv().await, None);
}

#[tokio::test]
async fn 방송_중이_아닌_채널은_구독할_수_없다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = service.list_channels().await.unwrap()[0].id;

    assert!(service.subscribe_chat(channel).await.is_err());
}

#[tokio::test]
async fn 구독하면_최근_50개를_먼저_받고_이어서_새_메시지를_받는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = open(dir.path()).await;
    let channel = go_live(&service, 0).await;
    for i in 0..60 {
        service
            .send_chat(channel, "철수", &format!("메시지 {i}"))
            .await
            .unwrap();
    }

    let mut sub = service.subscribe_chat(channel).await.unwrap();
    let next = service.send_chat(channel, "철수", "그 다음").await.unwrap();

    assert_eq!(sub.recent.len(), 50);
    assert_eq!(sub.recent[0].content, "메시지 10");
    assert_eq!(sub.recent[49].content, "메시지 59");
    assert_eq!(sub.recv().await, Some(ChatServerMsg::Message(next)));
}

#[tokio::test]
async fn 구독과_동시에_보내도_메시지가_빠지거나_겹치지_않는다() {
    let dir = tempfile::tempdir().unwrap();
    let service = std::sync::Arc::new(open(dir.path()).await);
    let channel = go_live(&service, 0).await;
    let sender = {
        let service = service.clone();
        tokio::spawn(async move {
            for i in 0..100 {
                service
                    .send_chat(channel, "철수", &format!("{i}"))
                    .await
                    .unwrap();
                tokio::task::yield_now().await;
            }
        })
    };
    let mut sub = service.subscribe_chat(channel).await.unwrap();
    sender.await.unwrap();

    let mut ids: Vec<i64> = sub.recent.iter().map(|m| m.id).collect();
    while let Ok(Some(ChatServerMsg::Message(m))) =
        tokio::time::timeout(Duration::from_millis(100), sub.recv()).await
    {
        ids.push(m.id);
    }

    let last = *ids.last().unwrap();
    let expected: Vec<i64> = (last - (ids.len() as i64 - 1)..=last).collect();
    assert_eq!(ids, expected, "연속된 아이디가 한 번씩만 와야 한다");
    assert_eq!(last, 100);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn 종료와_동시에_구독해도_구독은_종료를_받거나_거부된다() {
    let dir = tempfile::tempdir().unwrap();
    let service = std::sync::Arc::new(open(dir.path()).await);
    let channel = service.list_channels().await.unwrap()[0].id;
    for round in 0..300 {
        service
            .start_broadcast(SEED_STREAM_KEYS[0], "")
            .await
            .unwrap();
        let subscribers: Vec<_> = (0..4)
            .map(|_| {
                let service = service.clone();
                tokio::spawn(async move { service.subscribe_chat(channel).await })
            })
            .collect();
        let ender = {
            let service = service.clone();
            tokio::spawn(async move { service.end_broadcast(SEED_STREAM_KEYS[0]).await })
        };
        ender.await.unwrap().unwrap();
        for subscriber in subscribers {
            // 구독이 거부되는 것은 괜찮다. 성공했다면 반드시 종료를 받아야 한다.
            if let Ok(mut sub) = subscriber.await.unwrap() {
                let got = tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        match sub.recv().await {
                            Some(ChatServerMsg::Ended) => return true,
                            Some(_) => continue,
                            None => return false,
                        }
                    }
                })
                .await;
                assert_eq!(got, Ok(true), "{round}번째 방송: 종료를 받지 못했다");
            }
        }
    }
}