use api::ChannelStatus;
use dioxus::prelude::*;

use crate::views::browser::sleep_ms;

/// 상태를 다시 묻는 간격(밀리초).
const POLL_MS: u32 = 2000;

/// 채널의 방송·영상 신호 상태를 주기적으로 물어 따라간다. 컴포넌트가 사라지면 멈춘다.
/// 첫 응답이 오기 전에는 `initial`(없으면 `None`)이다. 요청이 실패하면 직전 값을 유지한다.
pub fn use_channel_status(id: i64, initial: Option<ChannelStatus>) -> Signal<Option<ChannelStatus>> {
    let mut status = use_signal(move || initial.clone());
    use_effect(move || {
        spawn(async move {
            loop {
                refresh_status(&mut status, id).await;
                sleep_ms(POLL_MS).await;
            }
        });
    });
    status
}

/// 지금 상태를 한 번 물어 반영한다. 방송 시작·종료 직후 화면을 바로 맞출 때도 쓴다.
pub async fn refresh_status(status: &mut Signal<Option<ChannelStatus>>, id: i64) {
    if let Ok(next) = api::get_channel_status(id).await {
        if status.peek().as_ref() != Some(&next) {
            status.set(Some(next));
        }
    }
}
