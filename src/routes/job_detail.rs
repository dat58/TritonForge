//! Job detail page with live progress tracking and download support.

use crate::api::{
    cancel_job, download_model, get_job_config_pbtxt, get_job_logs, get_job_status,
    update_job_config_pbtxt,
};
use crate::app::Route;
use crate::components::ProgressBar;
use crate::models::job::{JobId, JobStatus};
use crate::routes::timer;
#[cfg(target_arch = "wasm32")]
use dioxus::document::eval;
use dioxus::prelude::*;
use std::time::Duration;

/// Detail view for a single conversion job.
#[component]
pub fn JobDetailPage(job_id: String) -> Element {
    let parsed_id = job_id.parse::<uuid::Uuid>().ok().map(JobId);
    let mut refresh_tick = use_signal(|| 0u32);
    let mut log_tick = use_signal(|| 0u32);
    let mut job_active = use_signal(|| false);

    let job = use_resource({
        let job_id = job_id.clone();
        move || {
            let id = job_id.clone();
            let _ = refresh_tick();
            async move { get_job_status(id).await }
        }
    });

    let nav = use_navigator();
    let mut downloading = use_signal(|| false);
    let mut download_error: Signal<Option<String>> = use_signal(|| None);
    let mut cancelling = use_signal(|| false);
    let mut show_logs = use_signal(|| false);
    let mut editing_config = use_signal(|| false);
    let mut config_buf = use_signal(String::new);
    let mut config_loaded = use_signal(|| false);
    let mut config_saving = use_signal(|| false);
    let mut config_load_error: Signal<Option<String>> = use_signal(|| None);
    let mut config_save_error: Signal<Option<String>> = use_signal(|| None);

    let logs = use_resource({
        let job_id = job_id.clone();
        move || {
            let id = job_id.clone();
            let should_show = show_logs();
            let _ = log_tick();
            async move {
                if should_show {
                    get_job_logs(id, 1_000).await.map(Some)
                } else {
                    Ok(None)
                }
            }
        }
    });

    use_effect(move || {
        let is_active = job_is_active(&job);

        if *job_active.peek() != is_active {
            job_active.set(is_active);
        }
    });

    use_future(move || async move {
        loop {
            timer::sleep(Duration::from_secs(2)).await;
            let should_refresh = *job_active.read();

            if should_refresh {
                *refresh_tick.write() += 1;
            }
        }
    });

    use_future(move || async move {
        loop {
            timer::sleep(Duration::from_secs(2)).await;
            let should_refresh_logs = *show_logs.read() && *job_active.read();

            if should_refresh_logs {
                *log_tick.write() += 1;
            }
        }
    });

    let Some(jid) = parsed_id else {
        return rsx! {
            div { class: "min-h-screen flex items-center justify-center",
                div { class: "text-center",
                    p { class: "text-rose-400 text-lg mb-3", "Invalid job ID" }
                    Link {
                        to: Route::Jobs {},
                        class: "text-cyan-400 hover:text-cyan-300 transition-colors text-sm",
                        "← Back to Jobs"
                    }
                }
            }
        };
    };

    rsx! {
        div { class: "min-h-screen",
            div { class: "max-w-3xl mx-auto px-4 sm:px-6 py-14",

                // Back nav
                div { class: "mb-8",
                    Link {
                        to: Route::Jobs {},
                        class: "inline-flex items-center gap-1.5 text-slate-500 hover:text-cyan-300 transition-colors text-sm",
                        "← All Jobs"
                    }
                }

                {match &*job.read() {
                    None => rsx! {
                        div { class: "flex items-center gap-3 text-slate-400 py-12",
                            div { class: "w-5 h-5 rounded-full border-2 border-cyan-500 border-t-transparent animate-spin" }
                            "Loading job..."
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "rounded-xl p-6 text-rose-400 border border-rose-800/50 bg-rose-950/20",
                            "Failed to load job: {e}"
                        }
                    },
                    Some(Ok(j)) => {
                        let is_active = matches!(
                            j.status,
                            JobStatus::Pending
                                | JobStatus::Preparing
                                | JobStatus::Converting
                                | JobStatus::Finalizing
                        );
                        let is_done   = j.status == JobStatus::Completed;
                        let is_failed = j.status == JobStatus::Failed;
                        let model_name    = j.model_name.clone();
                        let model_version = j.model_version.to_string();
                        let jid_for_dl    = jid.clone();
                        let jid_for_cancel = jid.clone();
                        let fmt_str     = j.model_format.to_string();
                        let gpu_str     = format!("GPU {}", j.gpu_id);
                        let created_str = j.created_at.format("%Y-%m-%d %H:%M UTC").to_string();
                        let updated_str = j.updated_at.format("%Y-%m-%d %H:%M UTC").to_string();
                        let image_tag   = j.image_tag.clone();
                        let err_msg     = j.error_message.clone();
                        let jid_str     = jid.to_string();

                        rsx! {
                            div { class: "flex flex-col gap-5",

                                // ── Header card ──────────────────────────
                                div { class: "glass-card p-7",
                                    div { class: "flex items-start justify-between mb-5",
                                        div {
                                            h1 { class: "text-2xl font-bold text-slate-100 tracking-tight",
                                                "{model_name}"
                                            }
                                            p { class: "text-slate-500 text-xs mt-1 font-mono",
                                                "{jid_str}"
                                            }
                                        }
                                        {status_pill(is_done, is_failed, is_active)}
                                    }

                                    div { class: "grid grid-cols-2 sm:grid-cols-3 gap-4",
                                        {meta_cell("Model Name", &model_name)}
                                        {meta_cell("Version", &model_version)}
                                        {meta_cell("Format",   &fmt_str)}
                                        {meta_cell("GPU",      &gpu_str)}
                                        {meta_cell("Image",    &image_tag)}
                                        {meta_cell("Created",  &created_str)}
                                        {meta_cell("Updated",  &updated_str)}
                                    }
                                }

                                // ── Progress card ─────────────────────────
                                div { class: "glass-card p-7",
                                    h2 { class: "text-sm font-semibold uppercase tracking-wider text-slate-500 mb-5",
                                        "Progress"
                                    }
                                    ProgressBar { job: j.clone() }
                                    div { class: "mt-8 border-t border-slate-800/70 pt-5",
                                        button {
                                            r#type: "button",
                                            class: "flex items-center justify-between w-full px-3 py-2 rounded-lg text-sm text-slate-300 hover:text-cyan-300 hover:bg-slate-800/60 transition-colors",
                                            onclick: move |_| {
                                                show_logs.toggle();
                                                *log_tick.write() += 1;
                                            },
                                            span { "Container Logs" }
                                            span { if *show_logs.read() { "▲" } else { "▼" } }
                                        }
                                        if *show_logs.read() {
                                            div { class: "mt-3 rounded-lg border border-slate-800 bg-slate-950/60 overflow-hidden",
                                                {match &*logs.read() {
                                                    None => rsx! {
                                                        div { class: "px-3 py-3 text-sm text-slate-500", "Loading logs..." }
                                                    },
                                                    Some(Err(e)) => rsx! {
                                                        div { class: "px-3 py-3 text-sm text-rose-400", "Failed to load logs: {e}" }
                                                    },
                                                    Some(Ok(None)) => rsx! {
                                                        div { class: "px-3 py-3 text-sm text-slate-500", "Open logs to load container output." }
                                                    },
                                                    Some(Ok(Some(text))) => rsx! {
                                                        pre { class: "max-h-80 overflow-auto p-3 text-xs text-slate-300 whitespace-pre-wrap font-mono",
                                                            if text.trim().is_empty() {
                                                                "No container logs yet."
                                                            } else {
                                                                "{text}"
                                                            }
                                                        }
                                                    },
                                                }}
                                            }
                                        }
                                    }
                                }

                                // ── Download (completed) ──────────────────
                                if is_done {
                                    div { class: "glass-card p-5 border-emerald-800/40",
                                        if let Some(ref err) = *download_error.read() {
                                            div { class: "rounded-lg px-3 py-2.5 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30 mb-3",
                                                "{err}"
                                            }
                                        }
                                        div { class: "flex items-center gap-2",
                                            button {
                                                class: "flex-1 py-3.5 rounded-xl font-semibold text-sm text-white transition-all duration-200 disabled:opacity-50",
                                                style: "background: linear-gradient(135deg, #059669, #0d9488); box-shadow: 0 4px 16px rgba(5,150,105,0.3);",
                                                disabled: *downloading.read(),
                                                onclick: {
                                                    let jid_for_dl = jid_for_dl.clone();
                                                    let dl_name_root = model_name.clone();
                                                    move |_| {
                                                        let dl_id   = jid_for_dl.to_string();
                                                        let dl_name = dl_name_root.clone();
                                                        downloading.set(true);
                                                        download_error.set(None);
                                                        spawn(async move {
                                                            match download_model(dl_id).await {
                                                                Ok(bytes) => {
                                                                    trigger_browser_download(
                                                                        &bytes,
                                                                        &format!("{dl_name}.zip"),
                                                                    ).await;
                                                                    downloading.set(false);
                                                                }
                                                                Err(e) => {
                                                                    download_error.set(Some(e.to_string()));
                                                                    downloading.set(false);
                                                                }
                                                            }
                                                        });
                                                    }
                                                },
                                                if *downloading.read() {
                                                    div { class: "flex items-center justify-center gap-2",
                                                        div { class: "w-4 h-4 border-2 border-white/70 border-t-white rounded-full animate-spin" }
                                                        "Preparing download..."
                                                    }
                                                } else {
                                                    "↓  Download Model Folder"
                                                }
                                            }
                                            button {
                                                r#type: "button",
                                                class: "flex-shrink-0 w-11 h-11 rounded-xl text-slate-300 hover:text-cyan-300 bg-slate-800/60 hover:bg-slate-800 border border-slate-700 transition-colors text-base",
                                                title: "Edit config.pbtxt",
                                                onclick: {
                                                    let jid_for_edit = jid_for_dl.clone();
                                                    move |_| {
                                                        if *editing_config.read() {
                                                            editing_config.set(false);
                                                            return;
                                                        }
                                                        editing_config.set(true);
                                                        config_loaded.set(false);
                                                        config_load_error.set(None);
                                                        config_save_error.set(None);
                                                        let edit_id = jid_for_edit.to_string();
                                                        spawn(async move {
                                                            match get_job_config_pbtxt(edit_id).await {
                                                                Ok(text) => {
                                                                    config_buf.set(text);
                                                                    config_loaded.set(true);
                                                                }
                                                                Err(e) => {
                                                                    config_load_error.set(Some(e.to_string()));
                                                                    config_loaded.set(true);
                                                                }
                                                            }
                                                        });
                                                    }
                                                },
                                                "✎"
                                            }
                                        }
                                        if *editing_config.read() {
                                            div { class: "mt-4 rounded-xl border border-slate-800 bg-slate-950/60 p-3",
                                                if let Some(ref err) = *config_load_error.read() {
                                                    div { class: "rounded-lg px-3 py-2 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30 mb-3",
                                                        "Failed to load config.pbtxt: {err}"
                                                    }
                                                }
                                                if !*config_loaded.read() {
                                                    div { class: "px-3 py-3 text-sm text-slate-500",
                                                        "Loading config.pbtxt..."
                                                    }
                                                } else {
                                                    textarea {
                                                        class: "w-full h-[70vh] min-h-96 rounded-lg bg-slate-900 border border-slate-800 text-slate-200 text-xs font-mono p-3 focus:outline-none focus:border-cyan-700 overflow-auto",
                                                        spellcheck: "false",
                                                        wrap: "off",
                                                        value: "{config_buf.read()}",
                                                        oninput: move |evt| config_buf.set(evt.value()),
                                                    }
                                                    if let Some(ref err) = *config_save_error.read() {
                                                        div { class: "mt-3 rounded-lg px-3 py-2 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30",
                                                            "Save failed: {err}"
                                                        }
                                                    }
                                                    div { class: "flex items-center justify-end gap-2 mt-3",
                                                        button {
                                                            r#type: "button",
                                                            class: "px-4 py-2 rounded-lg text-sm text-slate-300 bg-slate-800 hover:bg-slate-700 border border-slate-700 transition-colors disabled:opacity-50",
                                                            disabled: *config_saving.read(),
                                                            onclick: move |_| {
                                                                editing_config.set(false);
                                                                config_save_error.set(None);
                                                            },
                                                            "Cancel"
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "px-4 py-2 rounded-lg text-sm font-semibold text-white bg-cyan-700 hover:bg-cyan-600 transition-colors disabled:opacity-50",
                                                            disabled: *config_saving.read(),
                                                            onclick: {
                                                                let jid_for_save = jid_for_dl.clone();
                                                                move |_| {
                                                                    let save_id = jid_for_save.to_string();
                                                                    let payload = config_buf.read().clone();
                                                                    config_saving.set(true);
                                                                    config_save_error.set(None);
                                                                    spawn(async move {
                                                                        match update_job_config_pbtxt(save_id, payload).await {
                                                                            Ok(()) => {
                                                                                config_saving.set(false);
                                                                                editing_config.set(false);
                                                                            }
                                                                            Err(e) => {
                                                                                config_save_error.set(Some(e.to_string()));
                                                                                config_saving.set(false);
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                            },
                                                            if *config_saving.read() { "Saving..." } else { "Save" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // ── Error (failed) ────────────────────────
                                if is_failed {
                                    div { class: "glass-card p-5 border-rose-800/40",
                                        if let Some(ref err) = err_msg {
                                            div { class: "rounded-lg px-4 py-3 text-rose-300 text-sm border border-rose-800/40 bg-rose-950/20 mb-4",
                                                strong { class: "font-semibold", "Error  " }
                                                "{err}"
                                            }
                                        }
                                        button {
                                            class: "w-full py-3 rounded-xl font-semibold text-sm text-slate-200 bg-slate-800 hover:bg-slate-700 border border-slate-700 transition-all duration-200",
                                            onclick: move |_| { let _ = nav.push(Route::Home {}); },
                                            "↩  Try Again"
                                        }
                                    }
                                }

                                // ── Cancel (active) ───────────────────────
                                if is_active {
                                    button {
                                        class: "w-full py-2.5 rounded-xl text-sm text-rose-500 hover:text-rose-300 border border-rose-900/60 hover:border-rose-700 hover:bg-rose-950/20 transition-all duration-200 disabled:opacity-40",
                                        disabled: *cancelling.read(),
                                        onclick: move |_| {
                                            let c_id = jid_for_cancel.to_string();
                                            cancelling.set(true);
                                            spawn(async move {
                                                let _ = cancel_job(c_id).await;
                                                *refresh_tick.write() += 1;
                                                cancelling.set(false);
                                            });
                                        },
                                        if *cancelling.read() { "Cancelling..." } else { "Cancel Job" }
                                    }
                                }
                            }
                        }
                    },
                }}
            }
        }
    }
}

fn status_pill(is_done: bool, is_failed: bool, is_active: bool) -> Element {
    if is_done {
        rsx! {
            span { class: "px-3 py-1 rounded-full text-xs font-semibold text-emerald-300",
                style: "background: rgba(5,150,105,0.15); border: 1px solid rgba(5,150,105,0.4);",
                "Completed"
            }
        }
    } else if is_failed {
        rsx! {
            span { class: "px-3 py-1 rounded-full text-xs font-semibold text-rose-300",
                style: "background: rgba(225,29,72,0.15); border: 1px solid rgba(225,29,72,0.4);",
                "Failed"
            }
        }
    } else if is_active {
        rsx! {
            span { class: "flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-semibold text-cyan-300",
                style: "background: rgba(8,145,178,0.15); border: 1px solid rgba(8,145,178,0.4);",
                div { class: "w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse" }
                "Running"
            }
        }
    } else {
        rsx! {
            span { class: "px-3 py-1 rounded-full text-xs font-semibold text-slate-400",
                style: "background: rgba(71,85,105,0.3); border: 1px solid rgba(71,85,105,0.5);",
                "Pending"
            }
        }
    }
}

fn job_is_active(job: &Resource<Result<crate::models::job::ConversionJob, ServerFnError>>) -> bool {
    job.read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .is_some_and(|job| is_active_status(&job.status))
}

fn is_active_status(status: &JobStatus) -> bool {
    matches!(
        status,
        JobStatus::Pending | JobStatus::Preparing | JobStatus::Converting | JobStatus::Finalizing
    )
}

fn meta_cell(label: &str, value: &str) -> Element {
    rsx! {
        div {
            p { class: "text-slate-600 text-xs mb-0.5 uppercase tracking-wide", "{label}" }
            p { class: "text-slate-300 text-sm break-all font-medium", "{value}" }
        }
    }
}

async fn trigger_browser_download(bytes: &[u8], filename: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let bytes_json = serde_json::to_string(bytes).unwrap_or_else(|_| "[]".to_string());
        let filename_json =
            serde_json::to_string(filename).unwrap_or_else(|_| "\"model.zip\"".to_string());
        let js = format!(
            "const b=new Uint8Array({bytes_json});
             const blob=new Blob([b]);
             const url=URL.createObjectURL(blob);
             const a=document.createElement('a');
             a.href=url; a.download={filename_json};
             document.body.appendChild(a); a.click();
             document.body.removeChild(a); URL.revokeObjectURL(url);"
        );
        let _ = eval(&js).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (bytes, filename);
    }
}
