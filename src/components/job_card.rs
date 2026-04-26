//! Summary card component for a single conversion job.

use crate::models::job::{ConversionJob, JobStatus};
use dioxus::prelude::*;

/// Summary card that links to the job detail page.
#[component]
pub fn JobCard(job: ConversionJob) -> Element {
    let (badge_bg, badge_text, badge_label) = status_badge(&job.status);
    let created = job.created_at.format("%b %d, %Y %H:%M").to_string();
    let job_id_str = job.id.to_string();
    let link_target = format!("/jobs/{job_id_str}");
    let pct = job.progress_percent;
    let bar_extra = if matches!(
        job.status,
        JobStatus::Preparing | JobStatus::Converting | JobStatus::Finalizing
    ) {
        " animation: pulse 2s ease-in-out infinite;"
    } else {
        ""
    };
    let bar_style =
        format!("width: {pct}%; background: linear-gradient(90deg, #0891b2, #0d9488);{bar_extra}");

    rsx! {
        a {
            href: "{link_target}",
            class: "block group",
            div {
                class: "glass-card p-5 hover:border-cyan-700/60 cursor-pointer",
                style: "transition: box-shadow 0.3s ease, border-color 0.3s ease;",

                // Header
                div { class: "flex items-start justify-between mb-4",
                    div { class: "flex-1 min-w-0 pr-3",
                        h3 {
                            class: "text-slate-100 font-semibold text-sm truncate group-hover:text-cyan-300 transition-colors duration-200",
                            "{job.model_name}"
                        }
                        p { class: "text-slate-500 text-xs mt-0.5", "{job.model_format}" }
                    }
                    span {
                        class: "flex-shrink-0 px-2.5 py-0.5 rounded-full text-xs font-medium {badge_bg} {badge_text}",
                        "{badge_label}"
                    }
                }

                // Progress
                div { class: "flex flex-col gap-1.5",
                    div { class: "flex items-center justify-between text-xs",
                        span { class: "text-slate-500", "Progress" }
                        span { class: "font-medium text-slate-300", "{pct}%" }
                    }
                    div { class: "w-full bg-slate-800 rounded-full h-1.5 overflow-hidden",
                        div { class: "h-1.5 rounded-full", style: "{bar_style}" }
                    }
                }

                p { class: "text-slate-600 text-xs mt-3", "{created}" }
            }
        }
    }
}

fn status_badge(status: &JobStatus) -> (&'static str, &'static str, &'static str) {
    match status {
        JobStatus::Pending => ("bg-slate-700", "text-slate-300", "Pending"),
        JobStatus::Preparing => ("bg-cyan-900/60", "text-cyan-300", "Preparing"),
        JobStatus::Converting => ("bg-cyan-900/60", "text-cyan-300", "Converting"),
        JobStatus::Finalizing => ("bg-cyan-900/60", "text-cyan-300", "Finalizing"),
        JobStatus::Completed => ("bg-emerald-900/60", "text-emerald-300", "Completed"),
        JobStatus::Failed => ("bg-rose-900/60", "text-rose-300", "Failed"),
    }
}
