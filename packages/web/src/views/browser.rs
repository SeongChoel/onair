//! 브라우저 쪽 도구 모음: localStorage, 타이머, 클립보드, 서버 오류 문구. 서비스는 이것을 모른다.
use api::WRONG_STREAM_KEY;
use dioxus::prelude::*;

/// 서버가 돌려준 거부 이유만 꺼낸다. 이유가 없는 오류는 그대로 보여 준다.
pub(crate) fn reason(e: ServerFnError) -> String {
    let text = e.to_string();
    let text = text
        .strip_prefix("error running server function: ")
        .unwrap_or(&text);
    text.strip_suffix(" (details: None)")
        .unwrap_or(text)
        .to_string()
}

/// 서버가 "송출 키가 틀렸다"고 답한 오류인가. 네트워크·서버 오류와 구분한다.
pub(crate) fn is_wrong_stream_key(e: &ServerFnError) -> bool {
    matches!(e, ServerFnError::ServerError { message, .. } if message == WRONG_STREAM_KEY)
}

/// JS 작은따옴표 문자열 안에 넣을 수 있게 이스케이프한다.
fn js_quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\'', "\\'")
}

/// localStorage 값 읽기. 없거나 빈 값이면 `None`.
pub(crate) async fn local_get(key: &str) -> Option<String> {
    let js = format!("return localStorage.getItem('{}');", js_quote(key));
    let value: serde_json::Value = document::eval(&js).await.ok()?;
    value.as_str().map(str::to_string).filter(|v| !v.is_empty())
}

pub(crate) fn local_set(key: &str, value: &str) {
    document::eval(&format!(
        "localStorage.setItem('{}', '{}');",
        js_quote(key),
        js_quote(value)
    ));
}

pub(crate) fn local_remove(key: &str) {
    document::eval(&format!("localStorage.removeItem('{}');", js_quote(key)));
}

pub(crate) fn copy_text(text: &str) {
    document::eval(&format!(
        "navigator.clipboard.writeText('{}');",
        js_quote(text)
    ));
}

/// 잠깐 기다린다. 브라우저 타이머를 쓰므로 컴포넌트가 사라지면 함께 취소된다.
pub(crate) async fn sleep_ms(ms: u32) {
    let _ = document::eval(&format!(
        "await new Promise(r => setTimeout(r, {ms})); return 1;"
    ))
    .await;
}
