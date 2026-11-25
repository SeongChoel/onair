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
    // MediaMTX 인증 경로 등은 이후 티켓에서 이 라우터에 붙인다.
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        api::server::init().await.map_err(std::io::Error::other)?;
        Ok(dioxus::server::router(App))
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
