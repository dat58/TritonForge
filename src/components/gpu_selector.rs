//! GPU device picker dropdown component.

use crate::api::get_available_gpus;
use crate::models::config::{GpuId, GpuInfo};
use dioxus::prelude::*;

/// Dropdown for selecting an available NVIDIA GPU, with a manual-entry fallback
/// when `nvidia-smi` is unavailable or reports no devices.
#[component]
pub fn GpuSelector(on_select: EventHandler<Option<GpuId>>, selected_gpu: Option<GpuId>) -> Element {
    let mut gpus: Signal<Option<Result<Vec<GpuInfo>, String>>> = use_signal(|| None);

    // use_effect is client-only (skipped during SSR), keeping the initial render tree
    // identical on both server and client so hydration assigns data-dioxus-id correctly.
    use_effect(move || {
        spawn(async move {
            let result = get_available_gpus().await.map_err(|e| e.to_string());
            gpus.set(Some(result));
        });
    });

    rsx! {
        div { class: "flex flex-col gap-1.5",
            label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                "GPU Device"
            }
            {match &*gpus.read() {
                None => rsx! { {loading_placeholder("Detecting GPUs…")} },
                Some(Ok(list)) if !list.is_empty() => rsx! {
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
                                "GPU {gpu.id.0}  ·  {gpu.name}  ·  {gpu.memory_free_mb} / {gpu.memory_total_mb} MB free"
                            }
                        }
                    }
                },
                Some(Ok(_)) => rsx! {
                    {info_box("No NVIDIA GPUs detected via nvidia-smi.")}
                    {manual_id_input(on_select, selected_gpu)}
                },
                Some(Err(_)) => rsx! {
                    {info_box("GPU auto-detection unavailable.")}
                    {manual_id_input(on_select, selected_gpu)}
                },
            }}
        }
    }
}

fn loading_placeholder(msg: &str) -> Element {
    rsx! {
        div { class: "field text-slate-500 animate-pulse", "{msg}" }
    }
}

fn info_box(msg: &str) -> Element {
    rsx! {
        div { class: "rounded-lg px-3 py-2 text-amber-400 text-xs border border-amber-800/50 bg-amber-950/30",
            "{msg}"
        }
    }
}

fn manual_id_input(on_select: EventHandler<Option<GpuId>>, selected_gpu: Option<GpuId>) -> Element {
    rsx! {
        div { class: "flex items-center gap-2",
            span { class: "text-slate-500 text-xs whitespace-nowrap", "GPU index:" }
            input {
                r#type: "number",
                class: "field",
                min: "0",
                max: "15",
                placeholder: "0",
                value: selected_gpu.map(|g| g.0.to_string()).unwrap_or_default(),
                oninput: move |evt| {
                    let selected = evt.value().trim().parse::<u32>().ok().map(GpuId);
                    on_select.call(selected);
                },
            }
        }
    }
}
