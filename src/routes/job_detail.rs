//! Job detail page with live progress tracking and download support.

use crate::api::{cancel_job, download_model, get_job_status};
use crate::app::Route;
use crate::components::ProgressBar;
use crate::models::job::{JobId, JobStatus};
use dioxus::prelude::*;

/// Detail view for a single conversion job.
///
/// Displays job metadata, a live `ProgressBar`, download button on completion,
/// and error details on failure.
#[component]
pub fn JobDetailPage(job_id: String) -> Element {
    let parsed_id = job_id.parse::<uuid::Uuid>().ok().map(JobId);

    let Some(jid) = parsed_id else {
        return rsx! {
            div { class: "min-h-screen bg-gray-950 text-gray-100 flex items-center justify-center",
                div { class: "text-center",
                    p { class: "text-red-400 text-lg", "Invalid job ID" }
                    Link {
                        to: Route::Jobs {},
                        class: "text-blue-400 hover:underline text-sm mt-2 block",
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
        div { class: "min-h-screen bg-gray-950 text-gray-100",
            div { class: "max-w-3xl mx-auto px-6 py-12",
                div { class: "mb-6",
                    Link {
                        to: Route::Jobs {},
                        class: "text-gray-400 hover:text-white text-sm transition-colors",
                        "← Back to Jobs"
                    }
                }

                {match &*job.read() {
                    None => rsx! {
                        div { class: "flex items-center gap-3 text-gray-400",
                            div { class: "w-5 h-5 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" }
                            "Loading job..."
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "bg-red-900/20 border border-red-700 rounded-xl p-6 text-red-400",
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
                        let is_done = j.status == JobStatus::Completed;
                        let is_failed = j.status == JobStatus::Failed;
                        let model_name = j.model_name.clone();
                        let jid_for_dl = jid.clone();
                        let jid_for_cancel = jid.clone();
                        let fmt_str = j.model_format.to_string();
                        let gpu_str = format!("GPU {}", j.gpu_id);
                        let created_str = j.created_at.format("%Y-%m-%d %H:%M UTC").to_string();
                        let updated_str = j.updated_at.format("%Y-%m-%d %H:%M UTC").to_string();
                        let image_tag = j.image_tag.clone();
                        let template_name = j.template_name.clone();
                        let output_path = j.output_path.as_ref().map(|p| p.display().to_string());
                        let error_message = j.error_message.clone();
                        let jid_display = jid.to_string();

                        rsx! {
                            div { class: "flex flex-col gap-6",
                                // Header
                                div { class: "bg-gray-900 border border-gray-800 rounded-2xl p-6",
                                    h1 { class: "text-2xl font-bold text-white mb-1", "{model_name}" }
                                    p { class: "text-gray-400 text-sm", "Job ID: {jid_display}" }

                                    div { class: "grid grid-cols-2 gap-4 mt-5 text-sm",
                                        {info_row("Format", &fmt_str)}
                                        {info_row("GPU", &gpu_str)}
                                        {info_row("Image", &image_tag)}
                                        {info_row("Template", &template_name)}
                                        {info_row("Created", &created_str)}
                                        {info_row("Updated", &updated_str)}
                                        if let Some(ref path) = output_path {
                                            {info_row("Output Path", path)}
                                        }
                                    }
                                }

                                // Progress
                                div { class: "bg-gray-900 border border-gray-800 rounded-2xl p-6",
                                    h2 { class: "text-lg font-semibold text-white mb-4", "Progress" }
                                    ProgressBar {
                                        job_id: jid.clone(),
                                        auto_refresh: is_active,
                                    }
                                }

                                // Download (completed)
                                if is_done {
                                    div { class: "flex flex-col gap-3",
                                        if let Some(ref err) = *download_error.read() {
                                            div { class: "bg-red-900/20 border border-red-700 rounded-lg px-4 py-3 text-red-400 text-sm",
                                                "{err}"
                                            }
                                        }
                                        button {
                                            class: "w-full py-3 bg-green-600 hover:bg-green-500 text-white rounded-xl font-semibold text-sm transition-colors disabled:opacity-50",
                                            disabled: *downloading.read(),
                                            onclick: move |_| {
                                                let dl_id = jid_for_dl.to_string();
                                                let dl_name = model_name.clone();
                                                downloading.set(true);
                                                download_error.set(None);
                                                spawn(async move {
                                                    match download_model(dl_id).await {
                                                        Ok(bytes) => {
                                                            trigger_browser_download(
                                                                &bytes,
                                                                &format!("{dl_name}.engine"),
                                                            )
                                                            .await;
                                                            downloading.set(false);
                                                        }
                                                        Err(e) => {
                                                            download_error
                                                                .set(Some(e.to_string()));
                                                            downloading.set(false);
                                                        }
                                                    }
                                                });
                                            },
                                            if *downloading.read() {
                                                div { class: "flex items-center justify-center gap-2",
                                                    div { class: "w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" }
                                                    "Preparing download..."
                                                }
                                            } else {
                                                "Download Engine File"
                                            }
                                        }
                                    }
                                }

                                // Error (failed)
                                if is_failed {
                                    div { class: "flex flex-col gap-3",
                                        if let Some(ref err) = error_message {
                                            div { class: "bg-red-900/20 border border-red-800 rounded-xl p-4 text-red-300 text-sm",
                                                strong { "Error: " }
                                                "{err}"
                                            }
                                        }
                                        button {
                                            class: "w-full py-3 bg-gray-700 hover:bg-gray-600 text-white rounded-xl font-semibold text-sm transition-colors",
                                            onclick: move |_| { let _ = nav.push(Route::Home {}); },
                                            "Try Again"
                                        }
                                    }
                                }

                                // Cancel (active)
                                if is_active {
                                    button {
                                        class: "w-full py-2 border border-red-700 text-red-400 hover:bg-red-900/20 rounded-xl text-sm transition-colors disabled:opacity-50",
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

fn info_row(label: &str, value: &str) -> Element {
    rsx! {
        div {
            p { class: "text-gray-500 text-xs mb-0.5", "{label}" }
            p { class: "text-gray-200 text-sm break-all", "{value}" }
        }
    }
}

/// Triggers a browser file download by injecting a temporary anchor element via JavaScript.
/// No-op on non-WASM targets where JavaScript is unavailable.
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
