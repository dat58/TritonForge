//! Root application component and route definitions.

use dioxus::prelude::*;

/// Top-level route enum for the application.
#[derive(Clone, Routable, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/jobs")]
    Jobs {},
    #[route("/jobs/:id")]
    JobDetail { id: String },
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
    rsx! {
        div {
            class: "min-h-screen bg-gray-950 text-gray-100 flex items-center justify-center",
            div {
                class: "text-center",
                h1 {
                    class: "text-4xl font-bold text-blue-400 mb-4",
                    "TritonForge"
                }
                p {
                    class: "text-gray-400 text-lg",
                    "TensorRT Model Converter"
                }
            }
        }
    }
}

#[component]
fn Jobs() -> Element {
    rsx! {
        div {
            class: "min-h-screen bg-gray-950 text-gray-100 p-8",
            h1 {
                class: "text-3xl font-bold mb-6",
                "Conversion Jobs"
            }
        }
    }
}

#[component]
fn JobDetail(id: String) -> Element {
    rsx! {
        div {
            class: "min-h-screen bg-gray-950 text-gray-100 p-8",
            h1 {
                class: "text-3xl font-bold mb-6",
                "Job: {id}"
            }
        }
    }
}
