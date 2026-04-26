//! Model upload and conversion submission form.

use crate::api::submit_job;
use crate::app::Route;
use crate::components::{GpuSelector, ImageSelector, TemplateSelector};
use crate::models::config::GpuId;
use crate::models::job::ModelFormat;
use dioxus::prelude::*;

/// Main upload form for submitting a new TensorRT conversion job.
///
/// Combines file selection, GPU/image/template pickers, and an optional
/// server output path into a single submission form.
#[component]
pub fn UploadForm() -> Element {
    let mut file_bytes: Signal<Option<Vec<u8>>> = use_signal(|| None);
    let mut file_name = use_signal(String::new);
    let mut model_format: Signal<Option<ModelFormat>> = use_signal(|| None);
    let mut selected_gpu: Signal<Option<GpuId>> = use_signal(|| None);
    let mut selected_image: Signal<Option<String>> = use_signal(|| None);
    let mut selected_template: Signal<Option<String>> = use_signal(|| None);
    let mut server_output_path = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    let nav = use_navigator();

    let can_submit = file_bytes.read().is_some()
        && model_format.read().is_some()
        && selected_gpu.read().is_some()
        && selected_image.read().is_some()
        && selected_template.read().is_some();

    rsx! {
        div { class: "flex flex-col gap-6",

            // ── File picker ──────────────────────────────────────────────
            div { class: "flex flex-col gap-2",
                label { class: "text-sm font-medium text-gray-300", "Model File" }
                label {
                    class: "flex flex-col items-center justify-center w-full h-32 border-2 border-dashed border-gray-600 rounded-xl cursor-pointer hover:border-blue-500 transition-colors bg-gray-800/50",
                    input {
                        r#type: "file",
                        class: "hidden",
                        accept: ".onnx,.pb,.savedmodel",
                        onchange: move |evt| {
                            let data = evt.data();
                            let files = data.files();
                            spawn(async move {
                                let Some(file) = files.into_iter().next() else { return };
                                let name = file.name();
                                let Ok(bytes) = file.read_bytes().await else { return };
                                file_name.set(name);
                                file_bytes.set(Some(bytes.to_vec()));
                            });
                        }
                    }
                    if file_bytes.read().is_some() {
                        div { class: "flex flex-col items-center gap-1",
                            span { class: "text-green-400 text-2xl", "✓" }
                            span { class: "text-gray-300 text-sm font-medium", "{file_name}" }
                            span { class: "text-gray-500 text-xs",
                                {
                                    let size_kb = file_bytes.read().as_ref().map(|b| b.len() / 1024).unwrap_or(0);
                                    format!("{size_kb} KB")
                                }
                            }
                        }
                    } else {
                        div { class: "flex flex-col items-center gap-1",
                            span { class: "text-gray-400 text-3xl", "↑" }
                            span { class: "text-gray-300 text-sm", "Click to select model file" }
                            span { class: "text-gray-500 text-xs", ".onnx  •  .pb  •  .savedmodel" }
                        }
                    }
                }
            }

            // ── Model format ─────────────────────────────────────────────
            div { class: "flex flex-col gap-2",
                label { class: "text-sm font-medium text-gray-300", "Model Format" }
                div { class: "flex gap-4",
                    for (label, fmt) in [("ONNX", ModelFormat::Onnx), ("TF SavedModel", ModelFormat::TensorFlowSavedModel)] {
                        label {
                            class: "flex items-center gap-2 cursor-pointer",
                            input {
                                r#type: "radio",
                                name: "model_format",
                                class: "accent-blue-500",
                                checked: *model_format.read() == Some(fmt.clone()),
                                onchange: {
                                    let fmt_clone = fmt.clone();
                                    move |_| model_format.set(Some(fmt_clone.clone()))
                                },
                            }
                            span { class: "text-gray-200 text-sm", "{label}" }
                        }
                    }
                }
            }

            // ── Selectors ────────────────────────────────────────────────
            GpuSelector {
                on_select: move |g| selected_gpu.set(g),
                selected_gpu: *selected_gpu.read(),
            }
            ImageSelector {
                on_select: move |i| selected_image.set(i),
                selected_image: selected_image.read().clone(),
            }
            TemplateSelector {
                on_select: move |t| selected_template.set(t),
                selected_template: selected_template.read().clone(),
                model_format: model_format.read().clone(),
            }

            // ── Optional server output path ──────────────────────────────
            div { class: "flex flex-col gap-1",
                label { class: "text-sm font-medium text-gray-300",
                    "Server Output Path "
                    span { class: "text-gray-500 font-normal", "(optional)" }
                }
                input {
                    r#type: "text",
                    class: "bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-gray-100 text-sm focus:outline-none focus:border-blue-500 placeholder-gray-500",
                    placeholder: "/data/models/my_model",
                    value: "{server_output_path}",
                    oninput: move |evt| server_output_path.set(evt.value()),
                }
            }

            // ── Error display ────────────────────────────────────────────
            if let Some(ref msg) = *error_msg.read() {
                div { class: "bg-red-900/20 border border-red-700 rounded-lg px-4 py-3 text-red-400 text-sm",
                    "{msg}"
                }
            }

            // ── Submit ───────────────────────────────────────────────────
            button {
                class: "w-full py-3 px-6 rounded-xl font-semibold text-sm transition-all {submit_btn_class(can_submit, *submitting.read())}",
                disabled: !can_submit || *submitting.read(),
                onclick: move |_| {
                    if !can_submit || *submitting.read() { return; }
                    let bytes = file_bytes.read().clone().unwrap_or_default();
                    let name = file_name.read().trim_end_matches(|c| ".onnx.pb.savedmodel".contains(c)).to_string();
                    let fmt = model_format.read().clone().unwrap();
                    let gpu = selected_gpu.read().unwrap();
                    let image = selected_image.read().clone().unwrap_or_default();
                    let template = selected_template.read().clone().unwrap_or_default();
                    let out_path = server_output_path.read().clone();
                    let out_path_opt = if out_path.trim().is_empty() { None } else { Some(out_path) };

                    submitting.set(true);
                    error_msg.set(None);

                    spawn(async move {
                        match submit_job(bytes, name, fmt, image, gpu.0, template, out_path_opt).await {
                            Ok(job_id) => {
                                nav.push(Route::JobDetail { id: job_id.to_string() });
                            }
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                                submitting.set(false);
                            }
                        }
                    });
                },
                if *submitting.read() {
                    div { class: "flex items-center justify-center gap-2",
                        div { class: "w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" }
                        "Submitting..."
                    }
                } else {
                    "Start Conversion"
                }
            }
        }
    }
}

fn submit_btn_class(can_submit: bool, submitting: bool) -> &'static str {
    if submitting {
        "bg-blue-700 text-white cursor-not-allowed opacity-70"
    } else if can_submit {
        "bg-blue-600 hover:bg-blue-500 text-white cursor-pointer"
    } else {
        "bg-gray-700 text-gray-400 cursor-not-allowed"
    }
}
