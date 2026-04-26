//! Animated progress bar that polls job status at a fixed interval.

use crate::api::get_job_status;
use crate::models::job::{ConversionJob, JobId, JobStatus};
use dioxus::prelude::*;
use futures_timer::Delay;
use std::time::Duration;

/// Visual progress bar for a conversion job.
///
/// When `auto_refresh` is true, polls `get_job_status` every 2 seconds
/// until the job reaches a terminal state (Completed or Failed).
#[component]
pub fn ProgressBar(job_id: JobId, auto_refresh: bool) -> Element {
    let mut job_data: Signal<Option<ConversionJob>> = use_signal(|| None);
    let mut poll_error: Signal<Option<String>> = use_signal(|| None);

    // Capture id_str before the closure so only a Clone-able String is moved.
    let id_str = job_id.to_string();
    use_coroutine(move |_rx: UnboundedReceiver<()>| {
        // Clone here (outside async) so the closure remains FnMut-compatible.
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

    let job = job_data.read();

    rsx! {
        div { class: "flex flex-col gap-3",
            if let Some(ref err) = *poll_error.read() {
                div { class: "text-red-400 text-sm", "Polling error: {err}" }
            }
            if let Some(ref j) = *job {
                {render_job_progress(j)}
            } else {
                div { class: "flex items-center gap-2 text-gray-400 text-sm",
                    div { class: "w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" }
                    "Loading job status..."
                }
            }
        }
    }
}

fn render_job_progress(job: &ConversionJob) -> Element {
    let (bar_color, status_color, label) = status_styles(&job.status);
    let pct = job.progress_percent;

    rsx! {
        div { class: "flex flex-col gap-2",
            div { class: "flex items-center justify-between text-sm",
                span { class: "font-medium {status_color}", "{label}" }
                span { class: "text-gray-400", "{pct}%" }
            }
            div { class: "w-full bg-gray-700 rounded-full h-3 overflow-hidden",
                div {
                    class: "h-3 rounded-full transition-all duration-500 {bar_color}",
                    style: "width: {pct}%",
                }
            }
            if let Some(ref err) = job.error_message {
                div { class: "text-red-400 text-sm mt-1", "{err}" }
            }
        }
    }
}

fn status_styles(status: &JobStatus) -> (&'static str, &'static str, &'static str) {
    match status {
        JobStatus::Pending => ("bg-gray-500", "text-gray-400", "Pending"),
        JobStatus::Preparing => ("bg-blue-500 animate-pulse", "text-blue-400", "Preparing"),
        JobStatus::Converting => ("bg-blue-500 animate-pulse", "text-blue-400", "Converting"),
        JobStatus::Finalizing => ("bg-blue-400 animate-pulse", "text-blue-300", "Finalizing"),
        JobStatus::Completed => ("bg-green-500", "text-green-400", "Completed"),
        JobStatus::Failed => ("bg-red-500", "text-red-400", "Failed"),
    }
}
