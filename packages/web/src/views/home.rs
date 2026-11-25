use dioxus::prelude::*;

use crate::views::ChannelAvatar;
use crate::Route;

/// 첫 화면. 모든 채널을 이름과 함께 보여 주고, 누르면 채널 페이지로 간다.
#[component]
pub fn Home() -> Element {
    let channels = use_server_future(api::list_channels)?;
    let list = match &*channels.read() {
        Some(Ok(list)) => list.clone(),
        Some(Err(_)) => {
            return rsx! { p { "채널 목록을 불러오지 못했습니다" } };
        }
        None => return rsx! { p { "불러오는 중" } },
    };

    rsx! {
        h1 { "채널" }
        ul { class: "channel-list",
            for channel in list {
                li { key: "{channel.id}",
                    Link {
                        class: "channel-card",
                        to: Route::Channel { id: channel.id },
                        ChannelAvatar { name: channel.name.clone(), image: channel.profile_image.clone() }
                        div { class: "channel-info",
                            span { class: "channel-name",
                                if channel.live_title.is_some() {
                                    span { class: "live-badge", "LIVE" }
                                }
                                "{channel.name}"
                            }
                            if let Some(title) = &channel.live_title {
                                span { class: "live-title", "{title}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
