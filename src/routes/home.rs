//! Home page: model upload and conversion submission form.

use crate::components::UploadForm;
use dioxus::prelude::*;

/// Home page that renders the model upload form.
#[component]
pub fn HomePage() -> Element {
    rsx! {
        div { class: "min-h-screen",
            div { class: "max-w-2xl mx-auto px-4 sm:px-6 py-14",

                // Page heading
                div { class: "mb-10 text-center",
                    div {
                        class: "inline-flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-medium mb-5",
                        style: "background: rgba(8,145,178,0.12); border: 1px solid rgba(8,145,178,0.3); color: #67e8f9;",
                        span { "⚡" }
                        "TensorRT Engine Builder"
                    }
                    h1 {
                        class: "text-3xl sm:text-4xl font-bold text-slate-100 mb-3 tracking-tight",
                        "New Conversion Job"
                    }
                    p { class: "text-slate-500 text-sm max-w-sm mx-auto",
                        "Upload an ONNX model and configure conversion settings. The engine will be built inside a Docker container."
                    }
                }

                // Form card
                div { class: "glass-card p-8",
                    UploadForm {}
                }
            }
        }
    }
}
