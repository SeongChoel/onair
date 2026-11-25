use api::ChannelStatus;
use dioxus::prelude::*;

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
}
