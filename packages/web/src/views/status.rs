use api::ChannelStatus;
use dioxus::prelude::*;

/// 상태를 다시 묻는 간격(밀리초).
const POLL_MS: u32 = 2000;

/// 채널의 방송·영상 신호 상태를 주기적으로 물어 따라간다. 컴포넌트가 사라지면 멈춘다.
/// 첫 응답이 오기 전에는 `initial`(없으면 `None`)이다. 요청이 실패하면 직전 값을 유지한다.
pub fn use_channel_status(id: i64, initial: Option<ChannelStatus>) -> Signal<Option<ChannelStatus>> {
    let mut status = use_signal(move || initial.clone());
    use_effect(move || {
        spawn(async move {
            loop {
                if let Ok(next) = api::get_channel_status(id).await {
                    if status.peek().as_ref() != Some(&next) {
                        status.set(Some(next));
                    }
                }
                // 브라우저 타이머로 기다린다. spawn은 컴포넌트와 함께 취소된다.
                let _ = document::eval(&format!(
                    "await new Promise((r) => setTimeout(r, {POLL_MS})); return 1;"
                ))
                .await;
            }
        });
    });
    status
}
