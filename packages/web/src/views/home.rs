use dioxus::prelude::*;

/// 첫 화면. 채널 목록은 이후 티켓에서 채운다.
#[component]
pub fn Home() -> Element {
    let channels = use_server_future(api::list_channels)?;
    let status = match &*channels.read() {
        Some(Ok(list)) => format!("채널 {}개", list.len()),
        Some(Err(_)) => "채널 목록을 불러오지 못했습니다".to_string(),
        None => "불러오는 중".to_string(),
    };

    rsx! {
        h1 { "onair" }
        p { "{status}" }
    }
}
