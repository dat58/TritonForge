//! Summary card component for a single conversion job.

use crate::app::Route;
use crate::models::job::{ConversionJob, JobId, JobStatus};
use dioxus::prelude::*;

/// Props for [`JobCard`].
#[derive(Props, Clone, PartialEq)]
pub struct JobCardProps {
    /// The job to display.
    pub job: ConversionJob,
    /// Called when the user confirms deletion.
    pub on_delete: EventHandler<JobId>,
}

/// Summary card that opens the job detail page and exposes job actions.
#[component]
pub fn JobCard(props: JobCardProps) -> Element {
    let nav = use_navigator();
    let mut confirm_delete = use_signal(|| false);
    let (badge_bg, badge_text, badge_label) = status_badge(&props.job.status);
    let created = props.job.created_at.format("%b %d, %Y %H:%M").to_string();
    let job_id = props.job.id.clone();
    let job_id_delete = props.job.id.clone();
    let pct = props.job.progress_percent;
    let can_delete = matches!(props.job.status, JobStatus::Completed | JobStatus::Failed);
    let bar_extra = if matches!(
        props.job.status,
        JobStatus::Preparing | JobStatus::Converting | JobStatus::Finalizing
    ) {
        " animation: pulse 2s ease-in-out infinite;"
    } else {
        ""
    };
    let bar_style =
        format!("width: {pct}%; background: linear-gradient(90deg, #0891b2, #0d9488);{bar_extra}");

    rsx! {
        div {
            class: "block group",
            onclick: move |_| {
                let _ = nav.push(Route::JobDetail { id: job_id.to_string() });
            },
            div {
                class: "glass-card p-5 hover:border-cyan-700/60 cursor-pointer",
                style: "transition: box-shadow 0.3s ease, border-color 0.3s ease;",

                // Header
                div { class: "flex items-start justify-between mb-4",
                    div { class: "flex-1 min-w-0 pr-3",
                        h3 {
                            class: "text-slate-100 font-semibold text-sm truncate group-hover:text-cyan-300 transition-colors duration-200",
                            "{props.job.model_name}"
                        }
                        p { class: "text-slate-500 text-xs mt-0.5",
                            "{props.job.model_format} · v{props.job.model_version}"
                        }
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

                div { class: "flex items-center justify-between gap-3 mt-3",
                    p { class: "text-slate-600 text-xs min-w-0 truncate", "{created}" }

                    if can_delete {
                        button {
                            r#type: "button",
                            class: "flex-shrink-0 px-3 py-1.5 rounded-lg text-xs font-medium bg-rose-900/40 hover:bg-rose-800/60 text-rose-300 border border-rose-800/50 transition-all duration-200",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                if *confirm_delete.read() {
                                    confirm_delete.set(false);
                                    props.on_delete.call(job_id_delete.clone());
                                } else {
                                    confirm_delete.set(true);
                                }
                            },
                            if *confirm_delete.read() {
                                "Confirm?"
                            } else {
                                "Delete"
                            }
                        }
                    }
                }
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
