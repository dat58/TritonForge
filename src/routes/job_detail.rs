//! Job detail page with live progress tracking and download support.

use crate::api::{cancel_job, download_model, get_job_status};
use crate::app::Route;
use crate::components::ProgressBar;
use crate::models::job::{JobId, JobStatus};
#[cfg(target_arch = "wasm32")]
use dioxus::document::eval;
use dioxus::prelude::*;

/// Detail view for a single conversion job.
#[component]
pub fn JobDetailPage(job_id: String) -> Element {
    let parsed_id = job_id.parse::<uuid::Uuid>().ok().map(JobId);

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

    let job = use_resource({
        let jid = jid.clone();
        move || {
            let id = jid.to_string();
            async move { get_job_status(id).await }
        }
    });

    let nav = use_navigator();
    let mut downloading = use_signal(|| false);
    let mut download_error: Signal<Option<String>> = use_signal(|| None);
    let mut cancelling = use_signal(|| false);

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
                        let jid_for_dl    = jid.clone();
                        let jid_for_cancel = jid.clone();
                        let fmt_str     = j.model_format.to_string();
                        let gpu_str     = format!("GPU {}", j.gpu_id);
                        let created_str = j.created_at.format("%Y-%m-%d %H:%M UTC").to_string();
                        let updated_str = j.updated_at.format("%Y-%m-%d %H:%M UTC").to_string();
                        let image_tag   = j.image_tag.clone();
                        let tmpl_name   = j.template_name.clone();
                        let out_path    = j.output_path.as_ref().map(|p| p.display().to_string());
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
                                        {meta_cell("Format",   &fmt_str)}
                                        {meta_cell("GPU",      &gpu_str)}
                                        {meta_cell("Image",    &image_tag)}
                                        {meta_cell("Template", &tmpl_name)}
                                        {meta_cell("Created",  &created_str)}
                                        {meta_cell("Updated",  &updated_str)}
                                        if let Some(ref path) = out_path {
                                            {meta_cell("Output Path", path)}
                                        }
                                    }
                                }

                                // ── Progress card ─────────────────────────
                                div { class: "glass-card p-7",
                                    h2 { class: "text-sm font-semibold uppercase tracking-wider text-slate-500 mb-5",
                                        "Progress"
                                    }
                                    ProgressBar {
                                        job_id: jid.clone(),
                                        auto_refresh: is_active,
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
                                        button {
                                            class: "w-full py-3.5 rounded-xl font-semibold text-sm text-white transition-all duration-200 disabled:opacity-50",
                                            style: "background: linear-gradient(135deg, #059669, #0d9488); box-shadow: 0 4px 16px rgba(5,150,105,0.3);",
                                            disabled: *downloading.read(),
                                            onclick: move |_| {
                                                let dl_id   = jid_for_dl.to_string();
                                                let dl_name = model_name.clone();
                                                downloading.set(true);
                                                download_error.set(None);
                                                spawn(async move {
                                                    match download_model(dl_id).await {
                                                        Ok(bytes) => {
                                                            trigger_browser_download(
                                                                &bytes,
                                                                &format!("{dl_name}.engine"),
                                                            ).await;
                                                            downloading.set(false);
                                                        }
                                                        Err(e) => {
                                                            download_error.set(Some(e.to_string()));
                                                            downloading.set(false);
                                                        }
                                                    }
                                                });
                                            },
                                            if *downloading.read() {
                                                div { class: "flex items-center justify-center gap-2",
                                                    div { class: "w-4 h-4 border-2 border-white/70 border-t-white rounded-full animate-spin" }
                                                    "Preparing download..."
                                                }
                                            } else {
                                                "↓  Download Engine File"
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
            serde_json::to_string(filename).unwrap_or_else(|_| "\"model.engine\"".to_string());
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
