//! Animated progress bar that polls job status at a fixed interval.

use crate::api::get_job_status;
use crate::models::job::{ConversionJob, JobId, JobStatus};
use dioxus::prelude::*;
use futures_timer::Delay;
use std::time::Duration;

/// Visual progress bar for a conversion job.
///
/// Polls `get_job_status` every 2 seconds when `auto_refresh` is true,
/// stopping when the job reaches a terminal state.
#[component]
pub fn ProgressBar(job_id: JobId, auto_refresh: bool) -> Element {
    let mut job_data: Signal<Option<ConversionJob>> = use_signal(|| None);
    let mut poll_error: Signal<Option<String>> = use_signal(|| None);

    let id_str = job_id.to_string();
    use_coroutine(move |_rx: UnboundedReceiver<()>| {
        let id = id_str.clone();
        async move {
            loop {
                match get_job_status(id.clone()).await {
                    Ok(job) => {
                        let terminal =
                            matches!(job.status, JobStatus::Completed | JobStatus::Failed);
                        job_data.set(Some(job));
                        if terminal || !auto_refresh {
                            break;
                        }
                    }
                    Err(e) => {
                        poll_error.set(Some(e.to_string()));
                        break;
                    }
                }
                if !auto_refresh {
                    break;
                }
                Delay::new(Duration::from_secs(2)).await;
            }
        }
    });

    rsx! {
        div { class: "flex flex-col gap-4",
            if let Some(ref err) = *poll_error.read() {
                div { class: "rounded-lg px-3 py-2.5 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30",
                    "Polling error: {err}"
                }
            }
            if let Some(ref j) = *job_data.read() {
                {render_job_progress(j)}
            } else {
                div { class: "flex items-center gap-3 text-slate-400 text-sm",
                    div {
                        class: "w-4 h-4 rounded-full border-2 border-cyan-500 border-t-transparent animate-spin flex-shrink-0"
                    }
                    "Loading job status..."
                }
            }
        }
    }
}

fn render_job_progress(job: &ConversionJob) -> Element {
    let (bar_color, label_class, label) = status_styles(&job.status);
    let pct = job.progress_percent;

    rsx! {
        div { class: "flex flex-col gap-3",
            div { class: "flex items-center justify-between",
                div { class: "flex items-center gap-2",
                    div { class: "w-2 h-2 rounded-full {bar_color}" }
                    span { class: "text-sm font-medium {label_class}", "{label}" }
                }
                span {
                    class: "text-sm font-semibold",
                    style: "color: #22d3ee;",
                    "{pct}%"
                }
            }
            // Progress track
            div { class: "w-full bg-slate-800 rounded-full h-2.5 overflow-hidden",
                div {
                    class: "h-2.5 rounded-full transition-all duration-700",
                    style: "width: {pct}%; background: linear-gradient(90deg, #0891b2, #0d9488);",
                }
            }
            if let Some(ref err) = job.error_message {
                div { class: "rounded-lg px-3 py-2 text-rose-400 text-xs border border-rose-800/40 bg-rose-950/20",
                    "{err}"
                }
            }
        }
    }
}

fn status_styles(status: &JobStatus) -> (&'static str, &'static str, &'static str) {
    match status {
        JobStatus::Pending => ("bg-slate-500", "text-slate-400", "Pending"),
        JobStatus::Preparing => ("bg-cyan-500 animate-pulse", "text-cyan-400", "Preparing"),
        JobStatus::Converting => ("bg-cyan-500 animate-pulse", "text-cyan-400", "Converting"),
        JobStatus::Finalizing => ("bg-cyan-400 animate-pulse", "text-cyan-300", "Finalizing"),
        JobStatus::Completed => ("bg-emerald-500", "text-emerald-400", "Completed"),
        JobStatus::Failed => ("bg-rose-500", "text-rose-400", "Failed"),
    }
}
