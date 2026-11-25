use dioxus::prelude::*;

/// 채널 프로필 이미지. 이미지가 없으면 기본 이미지(채널 이름 첫 글자)를 보여 준다.
#[component]
pub fn ChannelAvatar(name: String, image: Option<String>) -> Element {
    if let Some(src) = image {
        return rsx! {
            img { class: "avatar", src: "{src}", alt: "{name}" }
        };
    }
    let initial = name.chars().next().map(String::from).unwrap_or_default();
    rsx! {
        span { class: "avatar", "{initial}" }
    }
}
