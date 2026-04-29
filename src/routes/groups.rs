//! Groups management page — create, view, and manage model groups.

use crate::api::{
    add_models_to_group, create_model_group, delete_model_group, list_completed_jobs,
    list_model_groups, release_model_group, rename_model_group,
};
use crate::app::Route;
use crate::components::GroupCard;
use crate::models::group::{GroupId, ModelGroupMember};
use crate::models::job::ConversionJob;
use dioxus::prelude::*;
use std::collections::HashSet;

/// Model groups management page.
#[component]
pub fn GroupsPage() -> Element {
    let mut selected_group_id: Signal<Option<GroupId>> = use_signal(|| None);
    let checked_models: Signal<HashSet<String>> = use_signal(HashSet::new);
    let mut refresh_tick = use_signal(|| 0u32);
    let grouping_busy = use_signal(|| false);
    let mut create_busy = use_signal(|| false);

    let groups = use_resource(move || {
        let _ = refresh_tick();
        async move { list_model_groups().await }
    });

    let completed = use_resource(|| async move { list_completed_jobs().await });

    rsx! {
        div { class: "min-h-screen",
            div { class: "max-w-6xl mx-auto px-4 sm:px-6 py-14",

                // Header
                div { class: "flex items-center justify-between mb-10",
                    div {
                        h1 { class: "text-2xl sm:text-3xl font-bold text-slate-100 tracking-tight",
                            "Model Groups"
                        }
                        p { class: "text-slate-500 text-sm mt-1",
                            "Organise completed models into deployment groups"
                        }
                    }
                    div { class: "flex items-center gap-3",
                        button {
                            class: "flex items-center gap-1.5 px-3.5 py-2 rounded-lg text-sm text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition-all duration-200 border border-transparent hover:border-slate-700",
                            onclick: move |_| *refresh_tick.write() += 1,
                            "↻  Refresh"
                        }
                        button {
                            class: "flex items-center gap-1.5 px-3.5 py-2 rounded-lg text-sm font-medium text-white transition-all duration-200 disabled:opacity-50",
                            style: "background: linear-gradient(135deg, #0891b2, #0d9488); box-shadow: 0 2px 12px rgba(6,182,212,0.25);",
                            disabled: *create_busy.read(),
                            onclick: move |_| {
                                create_busy.set(true);
                                spawn(async move {
                                    if let Ok(group) = create_model_group(None).await {
                                        selected_group_id.set(Some(group.id));
                                        *refresh_tick.write() += 1;
                                    }
                                    create_busy.set(false);
                                });
                            },
                            if *create_busy.read() {
                                div { class: "w-3.5 h-3.5 rounded-full border-2 border-white border-t-transparent animate-spin" }
                            }
                            "+ Create Group"
                        }
                    }
                }

                // Groups grid
                {match &*groups.read() {
                    None => rsx! {
                        div { class: "flex items-center gap-3 text-slate-400 py-12",
                            div { class: "w-5 h-5 rounded-full border-2 border-cyan-500 border-t-transparent animate-spin" }
                            "Loading groups..."
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "rounded-xl p-6 text-rose-400 border border-rose-800/50 bg-rose-950/20",
                            "Failed to load groups: {e}"
                        }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        div { class: "flex flex-col items-center justify-center py-24 text-center",
                            div {
                                class: "w-16 h-16 rounded-2xl flex items-center justify-center mb-5",
                                style: "background: rgba(30,41,59,0.8); border: 1px solid #1e3a5f;",
                                span { class: "text-3xl text-slate-600", "⊞" }
                            }
                            p { class: "text-slate-400 text-lg font-medium mb-1", "No groups yet" }
                            p { class: "text-slate-600 text-sm",
                                "Click \"+ Create Group\" to get started."
                            }
                        }
                    },
                    Some(Ok(list)) => rsx! {
                        div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mb-12",
                            for group in list {
                                {
                                    let gid = group.id.clone();
                                    let gid_release = group.id.clone();
                                    let gid_release_check = group.id.clone();
                                    let gid_delete = group.id.clone();
                                    let gid_delete_check = group.id.clone();
                                    let selected = selected_group_id.read().as_ref() == Some(&gid);
                                    rsx! {
                                        GroupCard {
                                            group: group.clone(),
                                            selected,
                                            on_select: move |id: GroupId| selected_group_id.set(Some(id)),
                                            on_rename: move |(id, name): (GroupId, String)| {
                                                spawn(async move {
                                                    let _ = rename_model_group(id, name).await;
                                                    *refresh_tick.write() += 1;
                                                });
                                            },
                                            on_release: move |_: GroupId| {
                                                let id = gid_release.clone();
                                                let check = gid_release_check.clone();
                                                spawn(async move {
                                                    let _ = release_model_group(id).await;
                                                    *refresh_tick.write() += 1;
                                                    if selected_group_id.read().as_ref() == Some(&check) {
                                                        selected_group_id.set(None);
                                                    }
                                                });
                                            },
                                            on_delete: move |_: GroupId| {
                                                let id = gid_delete.clone();
                                                let check = gid_delete_check.clone();
                                                spawn(async move {
                                                    let _ = delete_model_group(id).await;
                                                    *refresh_tick.write() += 1;
                                                    if selected_group_id.read().as_ref() == Some(&check) {
                                                        selected_group_id.set(None);
                                                    }
                                                });
                                            },
                                        }
                                    }
                                }
                            }
                        }

                        // Add Models section — only show when there are groups
                        {add_models_section(
                            selected_group_id,
                            checked_models,
                            grouping_busy,
                            refresh_tick,
                            &completed,
                        )}
                    },
                }}
            }
        }
    }
}

fn add_models_section(
    selected_group_id: Signal<Option<GroupId>>,
    mut checked_models: Signal<HashSet<String>>,
    mut grouping_busy: Signal<bool>,
    mut refresh_tick: Signal<u32>,
    completed: &Resource<Result<Vec<ConversionJob>, ServerFnError>>,
) -> Element {
    let checked_count = checked_models.read().len();
    let has_selection = selected_group_id.read().is_some() && checked_count > 0;

    let button_label = if *grouping_busy.read() {
        "Adding…".to_owned()
    } else if checked_count > 0 {
        format!("Do Grouping Models ({checked_count} selected)")
    } else {
        "Do Grouping Models".to_owned()
    };

    rsx! {
        div { class: "border-t border-slate-800 pt-10",
            div { class: "flex items-center justify-between mb-6",
                div {
                    h2 { class: "text-lg font-semibold text-slate-200", "Add Models to Group" }
                    p { class: "text-slate-500 text-sm mt-0.5",
                        {if selected_group_id.read().is_some() {
                            "Check models below and click \"Do Grouping Models\" to copy them into the selected group."
                        } else {
                            "Select a group above first, then choose models to add."
                        }}
                    }
                }
                button {
                    class: "px-4 py-2 rounded-lg text-sm font-medium text-white transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed",
                    style: "background: linear-gradient(135deg, #0891b2, #0d9488);",
                    disabled: !has_selection || *grouping_busy.read(),
                    onclick: move |_| {
                        let Some(gid) = selected_group_id.read().clone() else { return; };
                        let members: Vec<ModelGroupMember> = checked_models
                            .read()
                            .iter()
                            .filter_map(|key| {
                                let (job_id, model_name) = key.split_once('/')?;
                                Some(ModelGroupMember {
                                    job_id: job_id.to_owned(),
                                    model_name: model_name.to_owned(),
                                })
                            })
                            .collect();
                        grouping_busy.set(true);
                        spawn(async move {
                            let _ = add_models_to_group(gid, members).await;
                            checked_models.write().clear();
                            grouping_busy.set(false);
                            *refresh_tick.write() += 1;
                        });
                    },
                    if *grouping_busy.read() {
                        div { class: "inline-block w-3.5 h-3.5 rounded-full border-2 border-white border-t-transparent animate-spin mr-1.5" }
                    }
                    "{button_label}"
                }
            }

            {match &*completed.read() {
                None => rsx! {
                    div { class: "flex items-center gap-3 text-slate-400 py-8",
                        div { class: "w-4 h-4 rounded-full border-2 border-cyan-500 border-t-transparent animate-spin" }
                        "Loading completed models..."
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "rounded-xl p-5 text-rose-400 border border-rose-800/50 bg-rose-950/20 text-sm",
                        "Failed to load completed models: {e}"
                    }
                },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    div { class: "text-slate-500 text-sm py-8 text-center",
                        "No completed models found."
                        br {}
                        Link {
                            to: Route::Home {},
                            class: "text-cyan-400 hover:text-cyan-300 transition-colors",
                            "Convert a model first →"
                        }
                    }
                },
                Some(Ok(list)) => rsx! {
                    div { class: "rounded-xl border border-slate-800 overflow-hidden",
                        // Table header
                        div { class: "grid grid-cols-[2rem_1fr_6rem_9rem] gap-x-4 px-4 py-2 bg-slate-800/60 text-xs font-semibold uppercase text-slate-500",
                            div {}
                            div { "Model" }
                            div { "Version" }
                            div { "Completed" }
                        }
                        // Rows
                        for job in list {
                            {
                                let key = format!("{}/{}", job.id, job.model_name);
                                let key_check = key.clone();
                                let checked = checked_models.read().contains(&key);
                                let created = job.created_at.format("%b %d, %Y").to_string();
                                rsx! {
                                    div {
                                        key: "{key}",
                                        class: "grid grid-cols-[2rem_1fr_6rem_9rem] gap-x-4 px-4 py-3 border-t border-slate-800/60 hover:bg-slate-800/30 transition-colors cursor-pointer items-center",
                                        onclick: move |_| {
                                            let mut models = checked_models.write();
                                            if models.contains(&key_check) {
                                                models.remove(&key_check);
                                            } else {
                                                models.insert(key_check.clone());
                                            }
                                        },
                                        div {
                                            class: "w-4 h-4 rounded border flex items-center justify-center flex-shrink-0",
                                            style: if checked { "background: #0891b2; border-color: #0891b2;" } else { "border-color: #475569;" },
                                            if checked {
                                                span { class: "text-white text-xs leading-none", "✓" }
                                            }
                                        }
                                        div { class: "min-w-0",
                                            p { class: "text-slate-200 text-sm font-medium truncate", "{job.model_name}" }
                                            p { class: "text-slate-500 text-xs", "{job.model_format}" }
                                        }
                                        div { class: "text-slate-400 text-sm", "v{job.model_version}" }
                                        div { class: "text-slate-500 text-xs", "{created}" }
                                    }
                                }
                            }
                        }
                    }
                },
            }}
        }
    }
}
