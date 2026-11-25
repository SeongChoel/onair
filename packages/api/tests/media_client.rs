//! MediaMTX 클라이언트 테스트. 진짜 MediaMTX 대신 요청을 기록하는 작은 HTTP 서버를 쓴다.
#![cfg(feature = "server")]

use std::sync::{Arc, Mutex};

use api::media::{MediaMtxClient, MediaServer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 요청 줄(`GET /path`)을 기록하고, 경로 조회에는 rtmpConn 송출을, 킥에는 `kick_status`를 돌려주는 가짜.
async fn fake_mediamtx(kick_status: u16) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = head.lines().next().unwrap_or("").to_string();
                let mut parts = line.split(' ');
                let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                log.lock().unwrap().push(format!("{method} {path}"));
                let (status, body) = if method == "GET" {
                    (200, r#"{"online":true,"source":{"type":"rtmpConn","id":"abc"}}"#.to_string())
                } else {
                    (kick_status, String::new())
                };
                let resp = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    (base, seen)
}

#[tokio::test]
async fn 송출을_끊을_때_mediamtx_킥_경로를_부른다() {
    let (base, seen) = fake_mediamtx(200).await;
    let client = MediaMtxClient::new(base);

    client.kick_publisher(7).await.unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen[0], "GET /v3/paths/get/live/7");
    assert_eq!(seen[1], "POST /v3/rtmp/conns/kick/abc");
}

#[tokio::test]
async fn 킥이_실패하면_http_상태가_이유에_담긴다() {
    let (base, _) = fake_mediamtx(500).await;
    let client = MediaMtxClient::new(base);

    let err = client.kick_publisher(7).await.unwrap_err();

    assert!(err.contains("500"), "{err}");
    assert!(err.contains("kick"), "{err}");
}
