//! TensorRT Docker image picker dropdown component.

use crate::api::get_available_images;
use crate::models::config::TensorRtImage;
use dioxus::prelude::*;

/// Dropdown for selecting a locally available TensorRT Docker image.
#[component]
pub fn ImageSelector(
    on_select: EventHandler<Option<String>>,
    selected_image: Option<String>,
) -> Element {
    let mut images: Signal<Option<Result<Vec<TensorRtImage>, String>>> = use_signal(|| None);

    // use_effect is client-only (skipped during SSR), keeping the initial render tree
    // identical on both server and client so hydration assigns data-dioxus-id correctly.
    use_effect(move || {
        spawn(async move {
            let result = get_available_images().await.map_err(|e| e.to_string());
            images.set(Some(result));
        });
    });

    rsx! {
        div { class: "flex flex-col gap-1.5",
            label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                "TensorRT Image"
            }
            {match &*images.read() {
                None => rsx! {
                    div { class: "field text-slate-500 animate-pulse", "Loading images..." }
                },
                Some(Err(e)) => rsx! {
                    div { class: "rounded-lg px-3 py-2.5 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30",
                        "Failed to load images: {e}"
                    }
                },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    div { class: "rounded-lg px-3 py-2.5 text-amber-400 text-sm border border-amber-800/50 bg-amber-950/30",
                        "No TensorRT images found locally. Run "
                        code { class: "font-mono text-amber-300 text-xs bg-amber-950 px-1 rounded",
                            "docker pull nvcr.io/nvidia/tensorrt:..."
                        }
                    }
                },
                Some(Ok(list)) => rsx! {
                    select {
                        class: "field",
                        onchange: move |evt| {
                            let val = evt.value();
                            let selected = if val.is_empty() { None } else { Some(val) };
                            on_select.call(selected);
                        },
                        option { value: "", "— Select Image —" }
                        for img in list {
                            option {
                                value: "{img.tag}",
                                selected: selected_image.as_deref() == Some(img.tag.as_str()),
                                "{img.name}  (TRT {img.tensorrt_version} / CUDA {img.cuda_version})"
                            }
                        }
                    }
                },
            }}
        }
    }
}
