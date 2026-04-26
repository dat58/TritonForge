//! Sticky top navigation bar with logo and page links.

use crate::app::Route;
use dioxus::prelude::*;

/// Top navigation bar with the app logo and route links.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        nav {
            class: "sticky top-0 z-50 bg-gray-900 border-b border-gray-800",
            div {
                class: "max-w-6xl mx-auto px-6 py-3 flex items-center justify-between",
                div {
                    class: "flex items-center gap-3",
                    span {
                        class: "text-blue-400 font-bold text-xl tracking-tight",
                        "TritonForge"
                    }
                    span {
                        class: "hidden sm:block text-gray-500 text-sm",
                        "TensorRT Converter"
                    }
                }
                div {
                    class: "flex items-center gap-6",
                    Link {
                        to: Route::Home {},
                        class: "text-gray-300 hover:text-white transition-colors text-sm font-medium",
                        "Upload"
                    }
                    Link {
                        to: Route::Jobs {},
                        class: "text-gray-300 hover:text-white transition-colors text-sm font-medium",
                        "Jobs"
                    }
                }
            }
        }
    }
}
