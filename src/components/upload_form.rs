//! Model upload and conversion submission form.

use crate::api::submit_job;
use crate::app::Route;
use crate::components::{GpuSelector, ImageSelector, TemplateSelector};
use crate::models::config::GpuId;
use crate::models::job::ModelFormat;
use dioxus::prelude::*;

/// Main upload form for submitting a new TensorRT conversion job.
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
        div { class: "flex flex-col gap-7",

            // ── File drop zone ───────────────────────────────────────────
            div { class: "flex flex-col gap-2",
                label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                    "Model File"
                }
                label {
                    class: "relative flex flex-col items-center justify-center w-full h-36 rounded-xl cursor-pointer border-2 border-dashed transition-all duration-300 group",
                    style: if file_bytes.read().is_some() {
                        "border-color: #0d9488; background: rgba(13,148,136,0.05);"
                    } else {
                        "border-color: #334155; background: rgba(30,41,59,0.3);"
                    },
                    onmouseenter: |_| {},
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
                        div { class: "flex flex-col items-center gap-1 pointer-events-none",
                            div {
                                class: "w-10 h-10 rounded-xl flex items-center justify-center mb-1",
                                style: "background: linear-gradient(135deg, #0891b2, #0d9488);",
                                span { class: "text-white text-lg", "✓" }
                            }
                            span { class: "text-teal-300 font-medium text-sm", "{file_name}" }
                            span { class: "text-slate-500 text-xs",
                                {
                                    let kb = file_bytes.read().as_ref().map(|b| b.len() / 1024).unwrap_or(0);
                                    format!("{kb} KB")
                                }
                            }
                        }
                    } else {
                        div { class: "flex flex-col items-center gap-1.5 pointer-events-none",
                            div { class: "w-10 h-10 rounded-xl bg-slate-800 flex items-center justify-center mb-1 group-hover:bg-slate-700 transition-colors",
                                span { class: "text-slate-400 text-xl group-hover:text-cyan-400 transition-colors", "↑" }
                            }
                            span { class: "text-slate-300 text-sm font-medium", "Click to select model file" }
                            span { class: "text-slate-600 text-xs", ".onnx  ·  .pb  ·  .savedmodel" }
                        }
                    }
                }
            }

            // ── Model format ─────────────────────────────────────────────
            div { class: "flex flex-col gap-2",
                label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                    "Model Format"
                }
                div { class: "flex gap-3",
                    for (lbl, fmt) in [("ONNX", ModelFormat::Onnx), ("TF SavedModel", ModelFormat::TensorFlowSavedModel)] {
                        label {
                            class: "flex items-center gap-2.5 cursor-pointer px-4 py-2.5 rounded-lg border transition-all duration-200",
                            style: if *model_format.read() == Some(fmt.clone()) {
                                "border-color: #0891b2; background: rgba(8,145,178,0.1); color: #67e8f9;"
                            } else {
                                "border-color: #334155; background: rgba(30,41,59,0.4); color: #94a3b8;"
                            },
                            input {
                                r#type: "radio",
                                name: "model_format",
                                class: "hidden",
                                checked: *model_format.read() == Some(fmt.clone()),
                                onchange: {
                                    let fmt_clone = fmt.clone();
                                    move |_| model_format.set(Some(fmt_clone.clone()))
                                },
                            }
                            span { class: "text-sm font-medium", "{lbl}" }
                        }
                    }
                }
            }

            // ── Selection section ─────────────────────────────────────────
            div {
                class: "rounded-xl border border-slate-800 p-4 flex flex-col gap-5",
                style: "background: rgba(15,23,42,0.5);",
                GpuSelector {
                    on_select: move |g| selected_gpu.set(g),
                    selected_gpu: *selected_gpu.read(),
                }
                div { class: "border-t border-slate-800/60" }
                ImageSelector {
                    on_select: move |i| selected_image.set(i),
                    selected_image: selected_image.read().clone(),
                }
                div { class: "border-t border-slate-800/60" }
                TemplateSelector {
                    on_select: move |t| selected_template.set(t),
                    selected_template: selected_template.read().clone(),
                    model_format: model_format.read().clone(),
                }
            }

            // ── Optional server path ─────────────────────────────────────
            div { class: "flex flex-col gap-1.5",
                label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                    "Server Output Path "
                    span { class: "normal-case font-normal text-slate-600", "(optional)" }
                }
                input {
                    r#type: "text",
                    class: "field",
                    placeholder: "/data/models/my_model",
                    value: "{server_output_path}",
                    oninput: move |evt| server_output_path.set(evt.value()),
                }
            }

            // ── Error ─────────────────────────────────────────────────────
            if let Some(ref msg) = *error_msg.read() {
                div { class: "rounded-lg px-4 py-3 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30",
                    "{msg}"
                }
            }

            // ── Checklist ─────────────────────────────────────────────────
            div { class: "grid grid-cols-2 gap-2 text-xs",
                for (done, lbl) in [
                    (file_bytes.read().is_some(),          "Model file"),
                    (model_format.read().is_some(),        "Format"),
                    (selected_gpu.read().is_some(),        "GPU"),
                    (selected_image.read().is_some(),      "Image"),
                    (selected_template.read().is_some(),   "Template"),
                ] {
                    div { class: "flex items-center gap-1.5",
                        span {
                            class: if done { "text-emerald-400" } else { "text-slate-600" },
                            if done { "✓" } else { "○" }
                        }
                        span {
                            class: if done { "text-slate-300" } else { "text-slate-600" },
                            "{lbl}"
                        }
                    }
                }
            }

            // ── Submit ────────────────────────────────────────────────────
            button {
                class: "w-full py-3.5 px-6 rounded-xl font-semibold text-sm transition-all duration-200",
                style: submit_btn_style(can_submit, *submitting.read()),
                disabled: !can_submit || *submitting.read(),
                onclick: move |_| {
                    if !can_submit || *submitting.read() { return; }
                    let bytes = file_bytes.read().clone().unwrap_or_default();
                    let name = {
                        let n = file_name.read();
                        let stripped = n.trim_end_matches(".onnx")
                            .trim_end_matches(".pb")
                            .trim_end_matches(".savedmodel")
                            .to_string();
                        if stripped.is_empty() { n.clone() } else { stripped }
                    };
                    let fmt  = model_format.read().clone().unwrap();
                    let gpu  = selected_gpu.read().unwrap();
                    let img  = selected_image.read().clone().unwrap_or_default();
                    let tmpl = selected_template.read().clone().unwrap_or_default();
                    let path = server_output_path.read().clone();
                    let path_opt = if path.trim().is_empty() { None } else { Some(path) };

                    submitting.set(true);
                    error_msg.set(None);

                    spawn(async move {
                        match submit_job(bytes, name, fmt, img, gpu.0, tmpl, path_opt).await {
                            Ok(job_id) => {
                                let _ = nav.push(Route::JobDetail { id: job_id.to_string() });
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
                        div { class: "w-4 h-4 border-2 border-white/70 border-t-white rounded-full animate-spin" }
                        "Submitting job..."
                    }
                } else {
                    "Start Conversion  →"
                }
            }
        }
    }
}

fn submit_btn_style(can_submit: bool, submitting: bool) -> &'static str {
    if submitting {
        "background: linear-gradient(135deg, #0e7490, #0f766e); color: white; opacity: 0.7; cursor: not-allowed;"
    } else if can_submit {
        "background: linear-gradient(135deg, #0891b2, #0d9488); color: white; cursor: pointer; box-shadow: 0 4px 20px rgba(6,182,212,0.3);"
    } else {
        "background: #1e293b; color: #475569; cursor: not-allowed;"
    }
}
