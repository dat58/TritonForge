//! Model upload and conversion submission form.

use crate::api::submit_job;
use crate::app::Route;
use crate::components::{GpuSelector, ImageSelector};
use crate::models::config::GpuId;
use crate::models::job::{SubmitJobRequest, TrtOptions};
use crate::onnx::{OnnxTensorInfo, parse_onnx_inputs};
use dioxus::prelude::*;

/// Main upload form for submitting a new TensorRT conversion job.
#[component]
pub fn UploadForm() -> Element {
    let mut file_bytes: Signal<Option<Vec<u8>>> = use_signal(|| None);
    let mut file_name = use_signal(String::new);
    let mut file_size: Signal<Option<u64>> = use_signal(|| None);
    let mut file_load_progress: Signal<Option<u8>> = use_signal(|| None);
    let mut file_load_error: Signal<Option<String>> = use_signal(|| None);
    let mut model_name = use_signal(String::new);
    let mut model_version = use_signal(|| 1u32);
    let mut selected_gpu: Signal<Option<GpuId>> = use_signal(|| None);
    let mut selected_image: Signal<Option<String>> = use_signal(|| None);
    let mut submitting = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    let mut explicit_batch = use_signal(|| true);
    let min_shapes = use_signal(String::new);
    let opt_shapes = use_signal(String::new);
    let max_shapes = use_signal(String::new);
    let mut workspace_mb = use_signal(|| 4096u32);
    let mut min_timing = use_signal(|| 8u32);
    let mut avg_timing = use_signal(|| 16u32);
    let mut fp16 = use_signal(|| true);
    let mut show_advanced = use_signal(|| false);
    let mut onnx_inputs: Signal<Vec<OnnxTensorInfo>> = use_signal(Vec::new);

    let nav = use_navigator();

    let can_submit = file_bytes.read().is_some()
        && !model_name.read().trim().is_empty()
        && *model_version.read() > 0
        && selected_gpu.read().is_some()
        && selected_image.read().is_some();

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
                    input {
                        r#type: "file",
                        class: "hidden",
                        accept: ".onnx",
                        onchange: move |evt| {
                            let data = evt.data();
                            let files = data.files();
                            file_load_error.set(None);
                            file_bytes.set(None);
                            file_load_progress.set(None);
                            onnx_inputs.set(Vec::new());
                            spawn(async move {
                                let Some(file) = files.into_iter().next() else { return };
                                let name = file.name();
                                let size = file.size();
                                file_name.set(name.clone());
                                file_size.set(Some(size));
                                file_load_progress.set(Some(0));
                                model_name.set(strip_onnx_extension(&name));

                                match read_selected_file(file, file_load_progress).await {
                                    Ok(bytes) => {
                                        file_load_progress.set(Some(100));
                                        let inputs =
                                            parse_onnx_inputs(&bytes).unwrap_or_default();
                                        onnx_inputs.set(inputs);
                                        file_bytes.set(Some(bytes));
                                    }
                                    Err(e) => {
                                        file_size.set(None);
                                        file_load_progress.set(None);
                                        file_load_error.set(Some(format!(
                                            "Could not read file: {e}"
                                        )));
                                    }
                                }
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
                                {format_file_size(file_size.read().unwrap_or(0))}
                            }
                        }
                    } else if let Some(progress) = *file_load_progress.read() {
                        div { class: "flex flex-col items-center gap-2 pointer-events-none w-full px-8",
                            div { class: "w-10 h-10 rounded-xl bg-slate-800 flex items-center justify-center mb-1",
                                div { class: "w-4 h-4 border-2 border-cyan-300/70 border-t-cyan-300 rounded-full animate-spin" }
                            }
                            span { class: "text-slate-300 font-medium text-sm", "{file_name}" }
                            span { class: "text-slate-500 text-xs",
                                "Reading {format_file_size(file_size.read().unwrap_or(0))}"
                            }
                            div { class: "w-full h-1.5 rounded-full bg-slate-800 overflow-hidden",
                                div {
                                    class: "h-full rounded-full bg-cyan-400 transition-all",
                                    style: "width: {progress}%;",
                                }
                            }
                        }
                    } else {
                        div { class: "flex flex-col items-center gap-1.5 pointer-events-none",
                            div { class: "w-10 h-10 rounded-xl bg-slate-800 flex items-center justify-center mb-1 group-hover:bg-slate-700 transition-colors",
                                span { class: "text-slate-400 text-xl group-hover:text-cyan-400 transition-colors", "↑" }
                            }
                            span { class: "text-slate-300 text-sm font-medium", "Click to select model file" }
                            span { class: "text-slate-600 text-xs", ".onnx" }
                        }
                    }
                }
                if let Some(ref msg) = *file_load_error.read() {
                    div { class: "rounded-lg px-3 py-2 text-rose-400 text-xs border border-rose-800/50 bg-rose-950/30",
                        "{msg}"
                    }
                }
            }

            // ── Model identity ───────────────────────────────────────────
            div { class: "grid grid-cols-1 sm:grid-cols-3 gap-4",
                div { class: "flex flex-col gap-1.5 sm:col-span-2",
                    label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                        "Model Name"
                    }
                    input {
                        r#type: "text",
                        class: "field",
                        placeholder: "resnet50",
                        value: "{model_name}",
                        oninput: move |evt| model_name.set(evt.value()),
                    }
                }
                div { class: "flex flex-col gap-1.5",
                    label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                        "Version"
                    }
                    input {
                        r#type: "number",
                        class: "field",
                        min: "1",
                        value: "{model_version}",
                        oninput: move |evt| {
                            if let Ok(value) = evt.value().parse::<u32>() {
                                model_version.set(value.max(1));
                            }
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
            }

            // ── Advanced Options Toggle ──────────────────────────────────
            button {
                r#type: "button",
                class: "flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-slate-500 hover:text-slate-300 transition-colors w-max",
                onclick: move |_| show_advanced.toggle(),
                span { if *show_advanced.read() { "▼" } else { "▶" } }
                "Advanced TensorRT Options"
            }

            if *show_advanced.read() {
                div { class: "flex flex-col gap-4 p-4 rounded-xl border border-slate-800 bg-slate-900/30",
                    div { class: "grid grid-cols-2 gap-4",
                        div { class: "flex flex-col gap-1.5",
                            label { class: "text-[10px] font-bold uppercase text-slate-500", "Workspace (MiB)" }
                            input {
                                r#type: "number",
                                class: "field text-sm py-1.5",
                                value: "{workspace_mb}",
                                oninput: move |evt| {
                                    if let Ok(val) = evt.value().parse::<u32>() {
                                        workspace_mb.set(val);
                                    }
                                }
                            }
                        }
                        div { class: "flex flex-col gap-1.5",
                            label { class: "text-[10px] font-bold uppercase text-slate-500", "Precision" }
                            div { class: "flex items-center gap-4 h-full",
                                label { class: "flex items-center gap-2 cursor-pointer text-sm text-slate-300",
                                    input {
                                        r#type: "checkbox",
                                        checked: *fp16.read(),
                                        onchange: move |_| fp16.toggle(),
                                    }
                                    "FP16"
                                }
                                label { class: "flex items-center gap-2 cursor-pointer text-sm text-slate-300",
                                    input {
                                        r#type: "checkbox",
                                        checked: *explicit_batch.read(),
                                        onchange: move |_| explicit_batch.toggle(),
                                    }
                                    "Explicit Batch"
                                }
                            }
                        }
                    }

                    div { class: "grid grid-cols-3 gap-3",
                        {
                            let ph_min = make_shapes_hint(&onnx_inputs.read(), 1);
                            let ph_opt = make_shapes_hint(&onnx_inputs.read(), 4);
                            let ph_max = make_shapes_hint(&onnx_inputs.read(), 8);
                            rsx! {
                                for (lbl, mut sig, ph) in [
                                    ("Min Shapes", min_shapes, ph_min),
                                    ("Opt Shapes", opt_shapes, ph_opt),
                                    ("Max Shapes", max_shapes, ph_max),
                                ] {
                                    div { class: "flex flex-col gap-1.5",
                                        label { class: "text-[10px] font-bold uppercase text-slate-500", "{lbl}" }
                                        input {
                                            r#type: "text",
                                            class: "field text-xs py-1.5",
                                            placeholder: "{ph}",
                                            value: "{sig}",
                                            oninput: move |evt| sig.set(evt.value()),
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "grid grid-cols-2 gap-4",
                        div { class: "flex flex-col gap-1.5",
                            label { class: "text-[10px] font-bold uppercase text-slate-500", "Min Timing" }
                            input {
                                r#type: "number",
                                class: "field text-sm py-1.5",
                                value: "{min_timing}",
                                oninput: move |evt| {
                                    if let Ok(val) = evt.value().parse::<u32>() {
                                        min_timing.set(val);
                                    }
                                }
                            }
                        }
                        div { class: "flex flex-col gap-1.5",
                            label { class: "text-[10px] font-bold uppercase text-slate-500", "Avg Timing" }
                            input {
                                r#type: "number",
                                class: "field text-sm py-1.5",
                                value: "{avg_timing}",
                                oninput: move |evt| {
                                    if let Ok(val) = evt.value().parse::<u32>() {
                                        avg_timing.set(val);
                                    }
                                }
                            }
                        }
                    }
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
                    (file_bytes.read().is_some(),        "Model file"),
                    (!model_name.read().trim().is_empty(), "Model name"),
                    (*model_version.read() > 0,          "Version"),
                    (selected_gpu.read().is_some(),      "GPU"),
                    (selected_image.read().is_some(),    "Image"),
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
                    let Some(bytes) = file_bytes.read().clone() else { return; };
                    let name = model_name.read().trim().to_owned();
                    let version = *model_version.read();
                    let Some(gpu) = *selected_gpu.read() else { return; };
                    let Some(img) = selected_image.read().clone() else { return; };

                    let trt_opts = TrtOptions {
                        explicit_batch: *explicit_batch.read(),
                        min_shapes: if min_shapes.read().trim().is_empty() { None } else { Some(min_shapes.read().clone()) },
                        opt_shapes: if opt_shapes.read().trim().is_empty() { None } else { Some(opt_shapes.read().clone()) },
                        max_shapes: if max_shapes.read().trim().is_empty() { None } else { Some(max_shapes.read().clone()) },
                        workspace_mb: *workspace_mb.read(),
                        min_timing: *min_timing.read(),
                        avg_timing: *avg_timing.read(),
                        fp16: *fp16.read(),
                    };

                    let req = SubmitJobRequest {
                        model_name: name,
                        model_version: version,
                        image_tag: img,
                        gpu_id: gpu.0,
                        trt_options: trt_opts,
                    };

                    submitting.set(true);
                    error_msg.set(None);

                    spawn(async move {
                        match submit_job(bytes, req).await {
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
                        "Submitting job…"
                    }
                } else {
                    "Start Conversion  →"
                }
            }
        }
    }
}

async fn read_selected_file(
    file: dioxus::html::FileData,
    mut progress: Signal<Option<u8>>,
) -> Result<Vec<u8>, dioxus::CapturedError> {
    progress.set(Some(10));
    file.read_bytes().await.map(|bytes| bytes.to_vec())
}

fn format_file_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes / 1024)
    }
}

fn strip_onnx_extension(name: &str) -> String {
    let stripped = name.trim_end_matches(".onnx");
    if stripped.is_empty() {
        name.to_owned()
    } else {
        stripped.to_owned()
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

/// Builds a trtexec shape hint string for the given batch size.
///
/// Returns an empty string when there are no inputs so the field shows no placeholder.
fn make_shapes_hint(inputs: &[OnnxTensorInfo], batch: i64) -> String {
    if inputs.is_empty() {
        return String::new();
    }
    inputs
        .iter()
        .map(|t| {
            let mut dims = t.dims.clone();
            if let Some(first) = dims.first_mut() {
                *first = batch;
            }
            let dim_str = dims
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("x");
            format!("{}:{}", t.name, dim_str)
        })
        .collect::<Vec<_>>()
        .join(",")
}
