//! Root application component, router definition, and shared layout.

use crate::components::Navbar;
use crate::routes::{HomePage, JobDetailPage, JobsPage};
use dioxus::prelude::*;

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
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// Shared layout that wraps all pages with the navigation bar.
#[component]
fn AppLayout() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-950",
            Navbar {}
            Outlet::<Route> {}
        }
    }
}

/// Root application component that mounts the router.
#[component]
pub fn App() -> Element {
    rsx! {
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
fn NotFound(segments: Vec<String>) -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-950 text-gray-100 flex items-center justify-center",
            div { class: "text-center",
                h1 { class: "text-6xl font-bold text-gray-700 mb-4", "404" }
                p { class: "text-gray-400 mb-6", "Page not found: /{segments.join(\"/\")}" }
                Link {
                    to: Route::Home {},
                    class: "text-blue-400 hover:underline",
                    "Go to Home"
                }
            }
        }
    }
}
