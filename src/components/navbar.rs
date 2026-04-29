//! Sticky top navigation bar with logo and page links.

use crate::app::Route;
use dioxus::prelude::*;

/// Glass navigation bar with gradient logo and animated link underlines.
#[component]
pub fn Navbar() -> Element {
    rsx! {
        nav {
            class: "sticky top-0 z-50 border-b border-slate-800/80 backdrop-blur-md",
            style: "background: rgba(2,6,23,0.85);",
            div {
                class: "max-w-6xl mx-auto px-6 py-4 flex items-center justify-between",

                // Logo
                Link {
                    to: Route::Home {},
                    class: "flex items-center gap-2.5 group",
                    div {
                        class: "w-8 h-8 rounded-lg flex items-center justify-center",
                        style: "background: linear-gradient(135deg, #0891b2, #0d9488);",
                        span { class: "text-white font-bold text-sm", "TF" }
                    }
                    div { class: "flex flex-col leading-none",
                        span {
                            class: "font-bold text-base tracking-tight",
                            style: "background: linear-gradient(90deg, #22d3ee, #2dd4bf); -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text;",
                            "TritonForge"
                        }
                        span { class: "text-slate-500 text-xs", "TensorRT Converter" }
                    }
                }

                // Nav links
                div { class: "flex items-center gap-8",
                    Link {
                        to: Route::Home {},
                        class: "link-nav",
                        "Upload"
                    }
                    Link {
                        to: Route::Jobs {},
                        class: "link-nav",
                        "Jobs"
                    }
                    Link {
                        to: Route::Groups {},
                        class: "link-nav",
                        "Groups"
                    }
                }
            }
        }
    }
}
