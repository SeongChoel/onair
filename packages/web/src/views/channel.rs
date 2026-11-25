use dioxus::prelude::*;

use crate::views::player::Player;
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
        match &channel.live_title {
            Some(title) => rsx! {
                p {
                    span { class: "live-badge", "LIVE" }
                    "{title}"
                }
                Player { channel_id: channel.id }
            },
            None => rsx! { p { class: "hint", "방송 중이 아닙니다" } },
        }
    }
}
