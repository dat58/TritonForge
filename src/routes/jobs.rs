//! Jobs list page with auto-refresh for in-progress conversions.

use crate::api::{delete_job, list_all_jobs};
use crate::app::Route;
use crate::components::JobCard;
use crate::models::job::{ConversionJob, JobId, JobStatus};
use crate::routes::timer;
use dioxus::prelude::*;
use std::time::Duration;

const PAGE_SIZE: u32 = 20;

/// Jobs history page — refreshes every 5 s while any job is active.
#[component]
pub fn JobsPage() -> Element {
    let mut page_offset = use_signal(|| 0u32);
    let mut poll_tick = use_signal(|| 0u32);
    let mut should_poll = use_signal(|| false);

    let jobs = use_resource(move || {
        let offset = page_offset();
        let _ = poll_tick();
        async move { list_all_jobs(PAGE_SIZE, offset).await }
    });

    use_effect(move || {
        let has_active = jobs
            .read()
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .is_some_and(|list| jobs_have_active(list));

        if *should_poll.peek() != has_active {
            should_poll.set(has_active);
        }
    });

    use_future(move || async move {
        loop {
            timer::sleep(Duration::from_secs(5)).await;
            let should_refresh = *should_poll.read();

            if should_refresh {
                *poll_tick.write() += 1;
            }
        }
    });

    let has_active_jobs = *should_poll.read();

    rsx! {
        div { class: "min-h-screen",
            div { class: "max-w-6xl mx-auto px-4 sm:px-6 py-14",

                // Header
                div { class: "flex items-center justify-between mb-10",
                    div {
                        h1 { class: "text-2xl sm:text-3xl font-bold text-slate-100 tracking-tight",
                            "Conversion Jobs"
                        }
                        p { class: "text-slate-500 text-sm mt-1",
                            "History of all TensorRT conversion runs"
                        }
                    }
                    div { class: "flex items-center gap-3",
                        // Refresh indicator when polling
                        if has_active_jobs {
                            div { class: "flex items-center gap-1.5 text-xs text-cyan-400",
                                div { class: "w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse" }
                                "Live"
                            }
                        }
                        button {
                            class: "flex items-center gap-1.5 px-3.5 py-2 rounded-lg text-sm text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-all duration-200 border border-transparent hover:border-slate-700",
                            onclick: move |_| *poll_tick.write() += 1,
                            "↻  Refresh"
                        }
                        Link {
                            to: Route::Home {},
                            class: "flex items-center gap-1.5 px-3.5 py-2 rounded-lg text-sm font-medium text-white transition-all duration-200",
                            style: "background: linear-gradient(135deg, #0891b2, #0d9488); box-shadow: 0 2px 12px rgba(6,182,212,0.25);",
                            "+ New Job"
                        }
                    }
                }

                // Content
                {match &*jobs.read() {
                    None => rsx! {
                        div { class: "flex items-center gap-3 text-slate-400 py-12",
                            div { class: "w-5 h-5 rounded-full border-2 border-cyan-500 border-t-transparent animate-spin" }
                            "Loading jobs..."
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "rounded-xl p-6 text-rose-400 border border-rose-800/50 bg-rose-950/20",
                            "Failed to load jobs: {e}"
                        }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        div { class: "flex flex-col items-center justify-center py-32 text-center",
                            div {
                                class: "w-16 h-16 rounded-2xl flex items-center justify-center mb-5",
                                style: "background: rgba(30,41,59,0.8); border: 1px solid #1e3a5f;",
                                span { class: "text-3xl text-slate-600", "⚗" }
                            }
                            p { class: "text-slate-400 text-lg font-medium mb-1",
                                "No conversions yet"
                            }
                            p { class: "text-slate-600 text-sm mb-6",
                                "Upload a model to kick off your first TensorRT build."
                            }
                            Link {
                                to: Route::Home {},
                                class: "px-5 py-2.5 rounded-xl text-sm font-semibold text-white",
                                style: "background: linear-gradient(135deg, #0891b2, #0d9488);",
                                "Upload a Model"
                            }
                        }
                    },
                    Some(Ok(list)) => rsx! {
                        div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4",
                            for job in list {
                                JobCard {
                                    job: job.clone(),
                                    on_delete: move |id: JobId| {
                                        spawn(async move {
                                            let _ = delete_job(id.to_string()).await;
                                            *poll_tick.write() += 1;
                                        });
                                    },
                                }
                            }
                        }
                        if list.len() == PAGE_SIZE as usize || *page_offset.read() > 0 {
                            div { class: "flex items-center justify-center gap-3 mt-8",
                                button {
                                    class: "px-4 py-2 rounded-lg text-sm text-slate-400 hover:text-slate-200 border border-slate-700 hover:border-slate-600 bg-slate-800/50 hover:bg-slate-800 transition-all disabled:opacity-30 disabled:cursor-not-allowed",
                                    disabled: *page_offset.read() == 0,
                                    onclick: move |_| {
                                        let cur = *page_offset.read();
                                        page_offset.set(cur.saturating_sub(PAGE_SIZE));
                                    },
                                    "← Previous"
                                }
                                span { class: "text-slate-600 text-sm",
                                    "Page {*page_offset.read() / PAGE_SIZE + 1}"
                                }
                                button {
                                    class: "px-4 py-2 rounded-lg text-sm text-slate-400 hover:text-slate-200 border border-slate-700 hover:border-slate-600 bg-slate-800/50 hover:bg-slate-800 transition-all disabled:opacity-30 disabled:cursor-not-allowed",
                                    disabled: list.len() < PAGE_SIZE as usize,
                                    onclick: move |_| {
                                        let cur = *page_offset.read();
                                        page_offset.set(cur + PAGE_SIZE);
                                    },
                                    "Next →"
                                }
                            }
                        }
                    },
                }}
            }
        }
    }
}

fn jobs_have_active(jobs: &[ConversionJob]) -> bool {
    jobs.iter().any(|job| is_active_status(&job.status))
}

fn is_active_status(status: &JobStatus) -> bool {
    !matches!(status, JobStatus::Completed | JobStatus::Failed)
}
