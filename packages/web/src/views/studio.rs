use api::StudioChannel;
use dioxus::prelude::*;

use crate::views::ChannelAvatar;

/// OBS 서버 주소. MediaMTX의 RTMP 수신 주소와 경로(`live`)다.
const OBS_SERVER: &str = "rtmp://localhost:1935/live";
/// 브라우저에 송출 키를 저장하는 localStorage 키.
const STORAGE_KEY: &str = "onair.streamKey";

/// 브라우저(웹 쪽)에서만 localStorage를 만진다. 서비스는 이것을 모른다.
async fn stored_key() -> Option<String> {
    let js = format!("return localStorage.getItem('{STORAGE_KEY}');");
    let value: serde_json::Value = document::eval(&js).await.ok()?;
    value.as_str().map(str::to_string).filter(|k| !k.is_empty())
}

fn js_quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\'', "\\'")
}

fn store_key(key: &str) {
    let key = js_quote(key);
    document::eval(&format!("localStorage.setItem('{STORAGE_KEY}', '{key}');"));
}

fn clear_key() {
    document::eval(&format!("localStorage.removeItem('{STORAGE_KEY}');"));
}

fn copy_text(text: &str) {
    let text = js_quote(text);
    document::eval(&format!("navigator.clipboard.writeText('{text}');"));
}

/// 스튜디오. 저장된 송출 키가 있으면 채널 관리 화면, 없으면 채널 만들기와 송출 키로 들어가기.
#[component]
pub fn Studio() -> Element {
    // None: 아직 브라우저 저장소를 확인하지 않음. Some(None): 들어가 있지 않음.
    let mut session = use_signal(|| None::<Option<StudioChannel>>);

    use_effect(move || {
        spawn(async move {
            let entered = match stored_key().await {
                Some(key) => api::enter_studio(key).await.ok(),
                None => None,
            };
            if entered.is_none() {
                clear_key();
            }
            session.set(Some(entered));
        });
    });

    let on_enter = move |channel: StudioChannel| {
        store_key(&channel.stream_key);
        session.set(Some(Some(channel)));
    };
    let on_leave = move |_| {
        clear_key();
        session.set(Some(None));
    };

    match session() {
        None => rsx! { p { "불러오는 중" } },
        Some(None) => rsx! {
            h1 { "스튜디오" }
            div { class: "studio-forms",
                CreateChannelForm { on_enter }
                EnterStudioForm { on_enter }
            }
        },
        Some(Some(channel)) => rsx! { ManageChannel { channel, on_leave } },
    }
}

#[component]
fn CreateChannelForm(on_enter: EventHandler<StudioChannel>) -> Element {
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        form {
            class: "studio-form",
            onsubmit: move |e| {
                e.prevent_default();
                spawn(async move {
                    match api::create_channel(name()).await {
                        Ok(channel) => on_enter.call(channel),
                        Err(e) => error.set(Some(reason(e))),
                    }
                });
            },
            h2 { "채널 만들기" }
            input {
                placeholder: "채널 이름",
                value: "{name}",
                oninput: move |e| name.set(e.value()),
            }
            button { r#type: "submit", "만들기" }
            if let Some(message) = error() {
                p { class: "error", "{message}" }
            }
        }
    }
}

#[component]
fn EnterStudioForm(on_enter: EventHandler<StudioChannel>) -> Element {
    let mut key = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        form {
            class: "studio-form",
            onsubmit: move |e| {
                e.prevent_default();
                spawn(async move {
                    match api::enter_studio(key().trim().to_string()).await {
                        Ok(channel) => on_enter.call(channel),
                        Err(e) => error.set(Some(reason(e))),
                    }
                });
            },
            h2 { "송출 키로 들어가기" }
            input {
                r#type: "password",
                placeholder: "송출 키",
                value: "{key}",
                oninput: move |e| key.set(e.value()),
            }
            button { r#type: "submit", "들어가기" }
            if let Some(message) = error() {
                p { class: "error", "{message}" }
            }
        }
    }
}

#[component]
fn ManageChannel(channel: StudioChannel, on_leave: EventHandler<()>) -> Element {
    // MediaMTX 발행 경로는 live/<채널 아이디>, 송출 키는 RTMP 쿼리로 간다.
    let obs_key = format!("{}?key={}", channel.id, channel.stream_key);
    let mut copied = use_signal(|| None::<&'static str>);

    rsx! {
        h1 { "스튜디오: {channel.name}" }
        BroadcastPanel {
            stream_key: channel.stream_key.clone(),
            initial_live_title: channel.live_title.clone(),
        }
        div { class: "obs-settings",
            h2 { "OBS 설정" }
            p { class: "hint",
                "OBS 설정 > 방송에서 서비스를 '사용자 지정'으로 두고 아래 두 값을 넣으세요. 스트림 키 칸에는 아래 문자열 전체를 넣습니다."
            }
            CopyField {
                label: "OBS 서버 주소",
                value: OBS_SERVER.to_string(),
                on_copy: move |_| copied.set(Some("서버 주소를 복사했습니다")),
            }
            CopyField {
                label: "OBS 스트림 키 (송출 키가 들어 있으니 남에게 보이지 마세요)",
                value: obs_key,
                on_copy: move |_| copied.set(Some("스트림 키를 복사했습니다")),
            }
            CopyField {
                label: "송출 키 (다른 브라우저에서 스튜디오에 들어갈 때 사용)",
                value: channel.stream_key.clone(),
                on_copy: move |_| copied.set(Some("송출 키를 복사했습니다")),
            }
            if let Some(message) = copied() {
                p { class: "hint", "{message}" }
            }
        }
        ProfileImage { channel: channel.clone() }
        button { class: "leave", onclick: move |_| on_leave.call(()), "나가기" }
    }
}

/// 프로필 이미지 보기와 올리기. 송출 키는 멀티파트 폼의 숨은 칸으로 간다.
#[component]
fn ProfileImage(channel: StudioChannel) -> Element {
    let id = channel.id;
    let mut current = use_resource(move || api::get_channel(id));
    let mut error = use_signal(|| None::<String>);
    let image = match &*current.read() {
        Some(Ok(c)) => c.profile_image.clone(),
        _ => None,
    };

    rsx! {
        div { class: "profile-image",
            h2 { "프로필 이미지" }
            ChannelAvatar { name: channel.name.clone(), image }
            form {
                onsubmit: move |evt: FormEvent| async move {
                    evt.prevent_default();
                    match api::upload_profile_image(evt.into()).await {
                        Ok(_) => {
                            error.set(None);
                            current.restart();
                        }
                        Err(e) => error.set(Some(reason(e))),
                    }
                },
                input { r#type: "hidden", name: "key", value: "{channel.stream_key}" }
                input {
                    r#type: "file",
                    name: "file",
                    accept: "image/png,image/jpeg,image/webp",
                }
                button { r#type: "submit", "올리기" }
            }
            p { class: "hint", "jpg, png, webp, 2MB 이하. 새로 올리면 기존 이미지가 바뀝니다." }
            if let Some(message) = error() {
                p { class: "error", "{message}" }
            }
        }
    }
}

/// 방송 시작·종료. 라이브 중이면 종료 버튼만, 아니면 제목 입력과 시작 버튼만 보인다.
#[component]
fn BroadcastPanel(stream_key: String, initial_live_title: Option<String>) -> Element {
    let mut live_title = use_signal(move || initial_live_title.clone());
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let start_key = stream_key.clone();

    rsx! {
        div { class: "broadcast-panel",
            h2 { "방송" }
            match live_title() {
                Some(current) => rsx! {
                    p {
                        span { class: "live-badge", "LIVE" }
                        "{current}"
                    }
                    button {
                        onclick: move |_| {
                            let key = stream_key.clone();
                            spawn(async move {
                                match api::end_broadcast(key).await {
                                    Ok(()) => {
                                        error.set(None);
                                        live_title.set(None);
                                    }
                                    Err(e) => error.set(Some(reason(e))),
                                }
                            });
                        },
                        "방송 종료"
                    }
                },
                None => rsx! {
                    form {
                        class: "studio-form",
                        onsubmit: move |e| {
                            e.prevent_default();
                            let key = start_key.clone();
                            spawn(async move {
                                match api::start_broadcast(key, title()).await {
                                    Ok(started) => {
                                        error.set(None);
                                        title.set(String::new());
                                        live_title.set(Some(started));
                                    }
                                    Err(e) => error.set(Some(reason(e))),
                                }
                            });
                        },
                        input {
                            placeholder: "방송 제목 (비우면 채널 이름의 방송)",
                            value: "{title}",
                            oninput: move |e| title.set(e.value()),
                        }
                        button { r#type: "submit", "방송 시작" }
                    }
                },
            }
            if let Some(message) = error() {
                p { class: "error", "{message}" }
            }
        }
    }
}

#[component]
fn CopyField(label: String, value: String, on_copy: EventHandler<()>) -> Element {
    let to_copy = value.clone();
    rsx! {
        div { class: "copy-field",
            label { "{label}" }
            div { class: "copy-row",
                input { readonly: true, value: "{value}" }
                button {
                    r#type: "button",
                    onclick: move |_| {
                        copy_text(&to_copy);
                        on_copy.call(());
                    },
                    "복사"
                }
            }
        }
    }
}

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
