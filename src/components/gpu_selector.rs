//! GPU device picker dropdown component.

use crate::api::get_available_gpus;
use crate::models::config::GpuId;
use dioxus::prelude::*;

/// Dropdown for selecting an available NVIDIA GPU.
#[component]
pub fn GpuSelector(on_select: EventHandler<Option<GpuId>>, selected_gpu: Option<GpuId>) -> Element {
    let gpus = use_resource(get_available_gpus);

    rsx! {
        div { class: "flex flex-col gap-1.5",
            label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                "GPU Device"
            }
            {match &*gpus.read() {
                None => rsx! { {skeleton_placeholder("Detecting GPUs...")} },
                Some(Err(e)) => rsx! { {error_box(&e.to_string())} },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    {info_box("No NVIDIA GPUs detected. Check nvidia-smi.")}
                },
                Some(Ok(list)) => rsx! {
                    select {
                        class: "field",
                        onchange: move |evt| {
                            let selected = if evt.value().is_empty() {
                                None
                            } else {
                                evt.value().parse::<u32>().ok().map(GpuId)
                            };
                            on_select.call(selected);
                        },
                        option { value: "", "— Select GPU —" }
                        for gpu in list {
                            option {
                                value: "{gpu.id.0}",
                                selected: selected_gpu == Some(gpu.id),
                                "{gpu.name}   ({gpu.memory_mb} MB VRAM)"
                            }
                        }
                    }
                },
            }}
        }
    }
}

fn skeleton_placeholder(msg: &str) -> Element {
    rsx! {
        div { class: "field text-slate-500 animate-pulse", "{msg}" }
    }
}

fn error_box(msg: &str) -> Element {
    rsx! {
        div { class: "rounded-lg px-3 py-2.5 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30",
            "{msg}"
        }
    }
}

fn info_box(msg: &str) -> Element {
    rsx! {
        div { class: "rounded-lg px-3 py-2.5 text-amber-400 text-sm border border-amber-800/50 bg-amber-950/30",
            "{msg}"
        }
    }
}
