use dioxus::prelude::*;

/// 프로필 이미지가 없는 채널의 기본 이미지: 채널 이름 첫 글자.
/// 프로필 이미지 표시는 티켓 09에서 붙인다.
#[component]
pub fn ChannelAvatar(name: String) -> Element {
    let initial = name.chars().next().map(String::from).unwrap_or_default();
    rsx! {
        span { class: "avatar", "{initial}" }
    }
}
