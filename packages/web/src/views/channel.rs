use api::{ChannelStatus, ChatMessage, ChatServerMsg};
use dioxus::fullstack::{use_websocket, WebSocketOptions};
use dioxus::prelude::*;

use crate::views::studio::reason;
use crate::views::player::Player;
use crate::views::status::use_channel_status;
use crate::views::ChannelAvatar;

/// 채널 페이지. 주소의 채널 아이디로 채널을 불러와 이름을 보여 준다.
#[component]
pub fn Channel(id: i64) -> Element {
    let channel = use_server_future(move || api::get_channel(id))?;
    let channel = match &*channel.read() {
        Some(Ok(channel)) => channel.clone(),
        Some(Err(_)) => return rsx! { p { "없는 채널입니다" } },
        None => return rsx! { p { "불러오는 중" } },
    };

    rsx! {
        div { class: "channel-header",
            ChannelAvatar { name: channel.name.clone(), image: channel.profile_image.clone() }
            h1 { "{channel.name}" }
        }
        LiveSection { channel_id: channel.id, initial_live_title: channel.live_title.clone() }
    }
}

/// 방송 상태에 따라 바뀌는 부분. 상태를 주기적으로 물어 새로고침 없이 따라간다.
/// 방송 중이면 제목과 플레이어, 신호가 없으면 "영상 신호 없음", 방송이 아니면 "방송 중이 아닙니다".
#[component]
fn LiveSection(channel_id: i64, initial_live_title: Option<String>) -> Element {
    let initial = ChannelStatus {
        live_title: initial_live_title,
        has_signal: false,
    };
    let status = use_channel_status(channel_id, Some(initial));
    let status = status().unwrap_or(ChannelStatus {
        live_title: None,
        has_signal: false,
    });

    let live = status.live_title.is_some();

    rsx! {
        match status.live_title {
            Some(title) => rsx! {
                p {
                    span { class: "live-badge", "LIVE" }
                    "{title}"
                }
                if status.has_signal {
                    Player { channel_id }
                } else {
                    div { class: "player",
                        div { class: "player-overlay", "영상 신호 없음" }
                    }
                }
            },
            None => rsx! { p { class: "hint", "방송 중이 아닙니다" } },
        }
        Chat { channel_id, live }
    }
}

/// 브라우저 localStorage에 닉네임을 저장하는 키. 서비스는 이것을 모른다.
const NICKNAME_KEY: &str = "onair.nickname";

async fn stored_nickname() -> Option<String> {
    let js = format!("return localStorage.getItem('{NICKNAME_KEY}');");
    let value: serde_json::Value = document::eval(&js).await.ok()?;
    value.as_str().map(str::to_string).filter(|n| !n.is_empty())
}

fn store_nickname(nickname: &str) {
    let nickname = nickname.replace('\\', "\\\\").replace('\'', "\\'");
    document::eval(&format!(
        "localStorage.setItem('{NICKNAME_KEY}', '{nickname}');"
    ));
}

/// 잠깐 기다린다. 브라우저 타이머를 쓴다.
async fn sleep_ms(ms: u32) {
    let _ = document::eval(&format!(
        "await new Promise(r => setTimeout(r, {ms})); return 1;"
    ))
    .await;
}

/// 채팅. 라이브 중이면 WebSocket을 열어 최근 메시지 50개를 받고 새 메시지를 새로고침 없이 받는다.
/// 방송이 종료되면 입력이 막힌다. 처음 보내려 하면 닉네임을 묻고, 닉네임은 브라우저에 저장한다.
#[component]
fn Chat(channel_id: i64, live: bool) -> Element {
    let mut messages = use_signal(Vec::<ChatMessage>::new);
    let mut ended = use_signal(|| false);
    // 소켓 연결이 `live`와 채널을 시그널로 읽어야 방송이 새로 시작될 때 다시 연결된다.
    let mut live_sig = use_signal(|| live);
    let mut channel_sig = use_signal(|| channel_id);
    let mut socket = use_websocket(move || {
        let _ = live_sig();
        api::chat_ws(channel_sig(), WebSocketOptions::new())
    });
    // 받기 반복. 시그널을 읽으면 다시 시작되므로 이 안에서는 `peek`만 쓰고, 다시 시작은 아래 이펙트가 맡는다.
    let mut receiver = use_future(move || async move {
        if !*live_sig.peek() {
            return;
        }
        loop {
            socket.connect().await;
            while let Ok(msg) = socket.recv().await {
                match msg {
                    ChatServerMsg::Recent(list) => {
                        ended.set(false);
                        messages.set(list);
                    }
                    ChatServerMsg::Message(m) => push_message(&mut messages, m),
                    ChatServerMsg::Ended => ended.set(true),
                }
            }
            if *ended.peek() {
                break;
            }
            // 연결이 끊겼거나 실패했다. 잠시 뒤 같은 연결을 다시 확인한다.
            sleep_ms(1000).await;
        }
    });
    use_effect(use_reactive!(|(live, channel_id)| {
        // 값이 바뀔 때만 건드린다. 시그널은 같은 값을 넣어도 알려서 소켓이 다시 연결된다.
        if *live_sig.peek() != live || *channel_sig.peek() != channel_id {
            live_sig.set(live);
            channel_sig.set(channel_id);
            receiver.restart();
        }
    }));
    // None: 저장소 확인 전. Some(None): 닉네임 없음. Some(Some(n)): 저장된 닉네임.
    let mut nickname = use_signal(|| None::<Option<String>>);
    let mut editing = use_signal(|| false);
    let mut nick_input = use_signal(String::new);
    let mut text = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        spawn(async move {
            let stored = stored_nickname().await;
            nick_input.set(stored.clone().unwrap_or_default());
            nickname.set(Some(stored));
        });
    });

    rsx! {
        div { class: "chat",
            h2 { "채팅" }
            ul { class: "chat-messages",
                for m in messages.read().iter() {
                    li { key: "{m.id}",
                        strong { "{m.nickname}" }
                        " {m.content}"
                    }
                }
            }
            if !live || ended() {
                p { class: "hint", "방송 중이 아니라 채팅할 수 없습니다" }
            } else if nickname().is_none() {
                p { class: "hint", "불러오는 중" }
            } else if nickname().flatten().is_none() || editing() {
                form {
                    class: "chat-form",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let value = nick_input().trim().to_string();
                        if value.is_empty() {
                            error.set(Some("닉네임을 입력하세요".to_string()));
                        } else if value.chars().count() > 20 {
                            error.set(Some("닉네임은 20자 이하여야 합니다".to_string()));
                        } else {
                            store_nickname(&value);
                            nickname.set(Some(Some(value)));
                            editing.set(false);
                            error.set(None);
                        }
                    },
                    input {
                        placeholder: "닉네임 (20자 이하)",
                        value: "{nick_input}",
                        oninput: move |e| nick_input.set(e.value()),
                    }
                    button { r#type: "submit", "닉네임 정하기" }
                }
            } else {
                form {
                    class: "chat-form",
                    onsubmit: move |e| {
                        e.prevent_default();
                        let Some(Some(nick)) = nickname() else { return };
                        spawn(async move {
                            match api::send_chat(channel_id, nick, text()).await {
                                Ok(m) => {
                                    error.set(None);
                                    text.set(String::new());
                                    // 소켓으로도 오지만 같은 메시지는 한 번만 보인다.
                                    push_message(&mut messages, m);
                                }
                                Err(e) => error.set(Some(reason(e))),
                            }
                        });
                    },
                    span { class: "hint",
                        "{nickname().flatten().unwrap_or_default()} "
                        button { r#type: "button", onclick: move |_| editing.set(true), "닉네임 바꾸기" }
                    }
                    input {
                        placeholder: "메시지 (500자 이하)",
                        value: "{text}",
                        oninput: move |e| text.set(e.value()),
                    }
                    button { r#type: "submit", "보내기" }
                }
            }
            if let Some(message) = error() {
                p { class: "error", "{message}" }
            }
        }
    }
}

/// 이미 있는 메시지는 건너뛰고 끝에 붙인다. 보낸 응답과 소켓 전달이 겹칠 수 있다.
fn push_message(messages: &mut Signal<Vec<ChatMessage>>, m: ChatMessage) {
    if !messages.peek().iter().any(|x| x.id == m.id) {
        messages.write().push(m);
    }
}
