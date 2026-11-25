use api::{ChannelStatus, StudioChannel};
use dioxus::prelude::*;

use crate::views::browser::{
    copy_text, is_wrong_stream_key, local_get, local_remove, local_set, reason,
};
use crate::views::status::{refresh_status, use_channel_status};
use crate::views::ChannelAvatar;

/// OBS 서버 주소. MediaMTX의 RTMP 수신 주소와 경로(`live`)다.
const OBS_SERVER: &str = "rtmp://localhost:1935/live";
/// 브라우저에 송출 키를 저장하는 localStorage 키.
const STORAGE_KEY: &str = "onair.streamKey";


/// 스튜디오. 저장된 송출 키가 있으면 채널 관리 화면, 없으면 채널 만들기와 송출 키로 들어가기.
#[component]
pub fn Studio() -> Element {
    // None: 아직 브라우저 저장소를 확인하지 않음. Some(None): 들어가 있지 않음.
    let mut entered = use_signal(|| None::<Option<StudioChannel>>);
    // 저장된 키로 들어가다 키 문제가 아닌 오류가 났을 때. 이때는 저장된 키를 지우지 않는다.
    let mut enter_error = use_signal(|| None::<String>);

    use_effect(move || {
        spawn(async move {
            let channel = match local_get(STORAGE_KEY).await {
                Some(key) => match api::enter_studio(key).await {
                    Ok(channel) => Some(channel),
                    Err(e) if is_wrong_stream_key(&e) => {
                        local_remove(STORAGE_KEY);
                        None
                    }
                    Err(e) => {
                        enter_error.set(Some(format!(
                            "저장된 송출 키로 들어가지 못했습니다: {} (새로고침하면 다시 시도합니다)",
                            reason(e)
                        )));
                        None
                    }
                },
                None => None,
            };
            entered.set(Some(channel));
        });
    });

    let on_enter = move |channel: StudioChannel| {
        local_set(STORAGE_KEY, &channel.stream_key);
        enter_error.set(None);
        entered.set(Some(Some(channel)));
    };
    let on_leave = move |_| {
        local_remove(STORAGE_KEY);
        entered.set(Some(None));
    };

    match entered() {
        None => rsx! { p { "불러오는 중" } },
        Some(None) => rsx! {
            h1 { "스튜디오" }
            if let Some(message) = enter_error() {
                p { class: "error", "{message}" }
            }
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
            channel_id: channel.id,
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

/// 방송 시작·종료. 상태를 주기적으로 물어 실제 방송을 따라가므로, 다른 곳에서 방송이 끝나도 반영된다.
/// 라이브 중이면 종료 버튼만, 아니면 제목 입력과 시작 버튼만 보인다.
#[component]
fn BroadcastPanel(channel_id: i64, stream_key: String, initial_live_title: Option<String>) -> Element {
    let initial = initial_live_title.map(|title| ChannelStatus {
        live_title: Some(title),
        broadcast_id: None,
        has_signal: false,
    });
    let mut status = use_channel_status(channel_id, initial);
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let start_key = stream_key.clone();

    rsx! {
        div { class: "broadcast-panel",
            h2 { "방송" }
            match status().and_then(|s| s.live_title.map(|t| (t, s.has_signal))) {
                Some((current, has_signal)) => rsx! {
                    p {
                        span { class: "live-badge", "LIVE" }
                        "{current}"
                    }
                    SignalIndicator { has_signal }
                    button {
                        onclick: move |_| {
                            let key = stream_key.clone();
                            spawn(async move {
                                match api::end_broadcast(key).await {
                                    Ok(()) => error.set(None),
                                    Err(e) => error.set(Some(reason(e))),
                                }
                                refresh_status(&mut status, channel_id).await;
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
                                    Ok(_) => {
                                        error.set(None);
                                        title.set(String::new());
                                    }
                                    Err(e) => error.set(Some(reason(e))),
                                }
                                refresh_status(&mut status, channel_id).await;
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

/// 영상 신호 있음/없음. 방송 중일 때만 그려진다.
#[component]
fn SignalIndicator(has_signal: bool) -> Element {
    if has_signal {
        rsx! { p { class: "signal signal-on", "영상 신호 있음" } }
    } else {
        rsx! {
            p { class: "signal signal-off",
                "영상 신호 없음 (OBS에서 송출을 시작하세요. 신호가 없어도 방송은 종료되지 않습니다)"
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
