//! GPU device picker dropdown component.

use crate::api::get_available_gpus;
use crate::models::config::GpuId;
use dioxus::prelude::*;

/// Dropdown for selecting an available NVIDIA GPU.
///
/// Calls `get_available_gpus` on mount and renders a select with GPU name and VRAM.
#[component]
pub fn GpuSelector(on_select: EventHandler<Option<GpuId>>, selected_gpu: Option<GpuId>) -> Element {
    let gpus = use_resource(get_available_gpus);

    rsx! {
        div { class: "flex flex-col gap-1",
            label { class: "text-sm font-medium text-gray-300", "GPU Device" }
            {match &*gpus.read() {
                None => rsx! {
                    div {
                        class: "bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-gray-400 text-sm animate-pulse",
                        "Detecting GPUs..."
                    }
                },
                Some(Err(e)) => rsx! {
                    div {
                        class: "bg-red-900/20 border border-red-700 rounded-lg px-3 py-2 text-red-400 text-sm",
                        "GPU detection failed: {e}"
                    }
                },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    div {
                        class: "bg-yellow-900/20 border border-yellow-700 rounded-lg px-3 py-2 text-yellow-400 text-sm",
                        "No NVIDIA GPUs detected"
                    }
                },
                Some(Ok(list)) => rsx! {
                    select {
                        class: "bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-gray-100 text-sm focus:outline-none focus:border-blue-500 cursor-pointer w-full",
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
                                "{gpu.name}  ({gpu.memory_mb} MB VRAM)"
                            }
                        }
                    }
                },
            }}
        }
    }
}
