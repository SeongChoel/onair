use dioxus::prelude::*;

/// hls.js (고정 버전). 브라우저 기본 HLS가 없는 Chrome/Edge/Firefox에서 쓴다.
const HLS_JS: &str = "https://cdn.jsdelivr.net/npm/hls.js@1.7.4/dist/hls.min.js";

/// 채널 아이디 경로(`live/<id>`)의 HLS를 재생한다. 송출 키는 주소에 나오지 않는다.
/// 영상 신호가 없으면(404 등) "영상 신호 없음"을 보이고, 2초마다 다시 붙는다.
#[component]
pub fn Player(channel_id: i64) -> Element {
    let mut playing = use_signal(|| false);
    let video_id = format!("player-{channel_id}");

    let eval_id = video_id.clone();
    use_effect(move || {
        let video_id = eval_id.clone();
        spawn(async move {
            // MediaMTX HLS 포트(8888)는 이 페이지와 같은 호스트에 있다.
            let mut ev = document::eval(&format!(
                r#"
                const video = document.getElementById("{video_id}");
                const url = location.protocol + "//" + location.hostname + ":8888/live/{channel_id}/index.m3u8";
                if (!window.Hls) {{
                  await new Promise((resolve) => {{
                    const s = document.createElement("script");
                    s.src = "{HLS_JS}";
                    s.onload = resolve; s.onerror = resolve;
                    document.head.appendChild(s);
                  }});
                }}
                let hls = null, timer = null, stopped = false;
                const stop = () => {{ stopped = true; if (hls) hls.destroy(); clearTimeout(timer); clearInterval(watch); }};
                // 컴포넌트가 사라지면(video가 DOM에서 빠지면) 정리한다.
                const watch = setInterval(() => {{ if (!video.isConnected) stop(); }}, 1000);
                const start = () => {{
                  if (stopped) return;
                  if (hls) {{ hls.destroy(); hls = null; }}
                  if (!window.Hls || !Hls.isSupported()) {{
                    video.src = url; video.play().catch(() => {{}}); dioxus.send("playing"); return;
                  }}
                  hls = new Hls({{ maxLiveSyncPlaybackRate: 1.5 }});
                  hls.on(Hls.Events.MANIFEST_LOADED, () => {{ dioxus.send("playing"); video.play().catch(() => {{}}); }});
                  hls.on(Hls.Events.ERROR, (_e, d) => {{
                    if (d.fatal) {{
                      dioxus.send("nosignal");
                      hls.destroy(); hls = null;
                      timer = setTimeout(start, 2000);
                    }}
                  }});
                  hls.loadSource(url);
                  hls.attachMedia(video);
                }};
                start();
                "#
            ));
            while let Ok(msg) = ev.recv::<String>().await {
                playing.set(msg == "playing");
            }
        });
    });

    rsx! {
        div { class: "player",
            video {
                id: "{video_id}",
                muted: true,
                autoplay: true,
                playsinline: true,
                controls: true,
            }
            if !playing() {
                div { class: "player-overlay", "영상 신호 없음" }
            }
        }
    }
}
