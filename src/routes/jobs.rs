//! Jobs list page with auto-refresh for in-progress conversions.

use crate::api::list_all_jobs;
use crate::components::JobCard;
use crate::models::job::JobStatus;
use dioxus::prelude::*;
use futures_timer::Delay;
use std::time::Duration;

const PAGE_SIZE: u32 = 20;

/// Jobs history page that shows all conversion jobs and refreshes every 5 seconds
/// when any job is still in progress.
#[component]
pub fn JobsPage() -> Element {
    let mut page_offset = use_signal(|| 0u32);
    let mut tick = use_signal(|| 0u32);
    let mut polling = use_signal(|| false);

    let jobs = use_resource(move || {
        let offset = page_offset();
        let _ = tick(); // reactive dep — re-runs when tick changes
        async move { list_all_jobs(PAGE_SIZE, offset).await }
    });

    // Auto-refresh: poll every 5 seconds when any job is active.
    use_effect(move || {
        let has_active = jobs
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|list| {
                list.iter()
                    .any(|j| !matches!(j.status, JobStatus::Completed | JobStatus::Failed))
            })
            .unwrap_or(false);

        if has_active && !*polling.read() {
            polling.set(true);
            spawn(async move {
                Delay::new(Duration::from_secs(5)).await;
                *tick.write() += 1;
                polling.set(false);
            });
        }
    });

    rsx! {
        div { class: "min-h-screen bg-gray-950 text-gray-100",
            div { class: "max-w-6xl mx-auto px-6 py-12",
                div { class: "flex items-center justify-between mb-8",
                    h1 { class: "text-3xl font-bold text-white", "Conversion Jobs" }
                    button {
                        class: "text-sm text-gray-400 hover:text-white transition-colors",
                        onclick: move |_| *tick.write() += 1,
                        "Refresh"
                    }
                }

                {match &*jobs.read() {
                    None => rsx! {
                        div { class: "flex items-center gap-3 text-gray-400",
                            div { class: "w-5 h-5 border-2 border-blue-500 border-t-transparent rounded-full animate-spin" }
                            "Loading jobs..."
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "bg-red-900/20 border border-red-700 rounded-xl p-6 text-red-400",
                            "Failed to load jobs: {e}"
                        }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        div { class: "flex flex-col items-center justify-center py-24 text-center",
                            p { class: "text-gray-400 text-lg", "No conversion jobs yet." }
                            Link {
                                to: crate::app::Route::Home {},
                                class: "text-blue-400 hover:underline text-sm mt-2",
                                "Upload a model to get started"
                            }
                        }
                    },
                    Some(Ok(list)) => rsx! {
                        div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4",
                            for job in list {
                                JobCard { job: job.clone() }
                            }
                        }
                        // Pagination
                        if list.len() == PAGE_SIZE as usize || *page_offset.read() > 0 {
                            div { class: "flex items-center justify-center gap-4 mt-8",
                                button {
                                    class: "px-4 py-2 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded-lg text-sm transition-colors disabled:opacity-40",
                                    disabled: *page_offset.read() == 0,
                                    onclick: move |_| {
                                        let cur = *page_offset.read();
                                        page_offset.set(cur.saturating_sub(PAGE_SIZE));
                                    },
                                    "← Previous"
                                }
                                button {
                                    class: "px-4 py-2 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded-lg text-sm transition-colors disabled:opacity-40",
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
