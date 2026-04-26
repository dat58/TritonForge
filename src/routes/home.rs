//! Home page: model upload and conversion submission form.

use crate::components::UploadForm;
use dioxus::prelude::*;

/// Home page that renders the model upload and conversion form.
#[component]
pub fn HomePage() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-950 text-gray-100",
            div { class: "max-w-2xl mx-auto px-6 py-12",
                div { class: "mb-8 text-center",
                    h1 { class: "text-3xl font-bold text-white mb-2", "New Conversion Job" }
                    p { class: "text-gray-400 text-sm",
                        "Upload a model file and configure conversion settings."
                    }
                }
                div { class: "bg-gray-900 border border-gray-800 rounded-2xl p-8",
                    UploadForm {}
                }
            }
        }
    }
}
