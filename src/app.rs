//! Root application component, router definition, and shared layout.

use crate::components::Navbar;
use crate::routes::{GroupsPage, HomePage, JobDetailPage, JobsPage};
use dioxus::prelude::*;

const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// Top-level route enum for the application.
#[derive(Clone, Routable, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppLayout)]
        #[route("/")]
        Home {},
        #[route("/jobs")]
        Jobs {},
        #[route("/jobs/:id")]
        JobDetail { id: String },
        #[route("/groups")]
        Groups {},
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// Shared layout that wraps all pages with the navigation bar.
#[component]
fn AppLayout() -> Element {
    rsx! {
        div { class: "min-h-screen",
            Navbar {}
            Outlet::<Route> {}
        }
    }
}

/// Root application component that mounts the router.
#[component]
pub fn App() -> Element {
    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Home() -> Element {
    rsx! { HomePage {} }
}

#[component]
fn Jobs() -> Element {
    rsx! { JobsPage {} }
}

#[component]
fn JobDetail(id: String) -> Element {
    rsx! { JobDetailPage { job_id: id } }
}

#[component]
fn Groups() -> Element {
    rsx! { GroupsPage {} }
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    rsx! {
        div { class: "min-h-screen flex items-center justify-center",
            div { class: "text-center",
                h1 { class: "text-8xl font-bold text-slate-800 mb-4", "404" }
                p { class: "text-slate-500 mb-6",
                    "Page not found: /{segments.join(\"/\")}"
                }
                Link {
                    to: Route::Home {},
                    class: "text-cyan-400 hover:text-cyan-300 transition-colors",
                    "← Go to Home"
                }
            }
        }
    }
}
