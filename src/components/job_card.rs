//! Summary card component for a single conversion job.

use crate::app::Route;
use crate::models::job::{ConversionJob, JobStatus};
use dioxus::prelude::*;

/// Summary card that links to the job detail page.
///
/// Displays model name, format, status badge, progress, and creation time.
#[component]
pub fn JobCard(job: ConversionJob) -> Element {
    let (badge_class, badge_label) = status_badge(&job.status);
    let created = job.created_at.format("%Y-%m-%d %H:%M UTC").to_string();
    let job_id_str = job.id.to_string();
    let pct = job.progress_percent;

    rsx! {
        Link {
            to: Route::JobDetail { id: job_id_str },
            class: "block",
            div {
                class: "bg-gray-800 border border-gray-700 rounded-xl p-5 hover:border-blue-600 transition-colors cursor-pointer",
                div { class: "flex items-start justify-between mb-3",
                    div {
                        h3 { class: "text-white font-semibold text-base truncate", "{job.model_name}" }
                        p { class: "text-gray-400 text-xs mt-0.5", "{job.model_format}" }
                    }
                    span { class: "px-2 py-0.5 rounded-full text-xs font-medium {badge_class}", "{badge_label}" }
                }
                div { class: "flex flex-col gap-1",
                    div { class: "flex items-center justify-between text-xs text-gray-400",
                        span { "Progress" }
                        span { "{pct}%" }
                    }
                    div { class: "w-full bg-gray-700 rounded-full h-1.5",
                        div {
                            class: "h-1.5 rounded-full bg-blue-500",
                            style: "width: {pct}%",
                        }
                    }
                }
                p { class: "text-gray-500 text-xs mt-3", "{created}" }
            }
        }
    }
}

fn status_badge(status: &JobStatus) -> (&'static str, &'static str) {
    match status {
        JobStatus::Pending => ("bg-gray-700 text-gray-300", "Pending"),
        JobStatus::Preparing => ("bg-blue-900/60 text-blue-300", "Preparing"),
        JobStatus::Converting => ("bg-blue-900/60 text-blue-300", "Converting"),
        JobStatus::Finalizing => ("bg-blue-900/60 text-blue-300", "Finalizing"),
        JobStatus::Completed => ("bg-green-900/60 text-green-300", "Completed"),
        JobStatus::Failed => ("bg-red-900/60 text-red-300", "Failed"),
    }
}
