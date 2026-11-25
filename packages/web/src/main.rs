use dioxus::prelude::*;

use ui::Navbar;
use views::{Channel, Home, Studio};

mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(WebNavbar)]
    #[route("/")]
    Home {},
    #[route("/channel/:id")]
    Channel { id: i64 },
    #[route("/studio")]
    Studio {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);

    // 서버: onair 서비스를 한 번만 만들고 Dioxus 라우터를 띄운다.
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use dioxus::server::axum::extract::DefaultBodyLimit;
        let service = api::server::init().await.map_err(std::io::Error::other)?;
        Ok(dioxus::server::router(App)
            // MediaMTX가 송출·재생 요청마다 부르는 확인 주소 (같은 PC에서만 온다).
            .merge(api::server::mediamtx_auth_router())
            // 업로드 이미지. 폴더 안 파일 이름에는 송출 키가 없다.
            .nest_service(
                "/uploads",
                tower_http::services::ServeDir::new(service.upload_dir()),
            )
            // 프로필 이미지 멀티파트(2MB + 오버헤드). 2MB 판정은 서비스가 한다.
            .layer(DefaultBodyLimit::max(4 * 1024 * 1024)))
    });
}

#[component]
fn App() -> Element {
    rsx! {
        document::Title { "onair" }
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        Router::<Route> {}
    }
}

/// 웹 전용 라우터를 감싸는 공통 `Navbar`.
#[component]
fn WebNavbar() -> Element {
    rsx! {
        Navbar {
            Link {
                to: Route::Home {},
                "onair"
            }
            Link {
                to: Route::Studio {},
                "스튜디오"
            }
        }

        Outlet::<Route> {}
    }
}
