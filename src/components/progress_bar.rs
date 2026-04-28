//! Pure visual progress bar for a conversion job.

use crate::models::job::{ConversionJob, JobStatus};
use dioxus::prelude::*;

/// Visual progress bar for a conversion job.
///
/// The containing page owns polling and passes the latest job snapshot.
#[component]
pub fn ProgressBar(job: ConversionJob) -> Element {
    rsx! {
        div { class: "flex flex-col gap-4",
            {render_job_progress(&job)}
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
