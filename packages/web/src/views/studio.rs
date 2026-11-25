use api::StudioChannel;
use dioxus::prelude::*;

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
        button { class: "leave", onclick: move |_| on_leave.call(()), "나가기" }
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
fn reason(e: ServerFnError) -> String {
    let text = e.to_string();
    let text = text
        .strip_prefix("error running server function: ")
        .unwrap_or(&text);
    text.strip_suffix(" (details: None)")
        .unwrap_or(text)
        .to_string()
}
