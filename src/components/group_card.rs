//! Card component for a model group with inline rename and action buttons.

use crate::api::{
    get_group_serving_logs, get_group_serving_status, start_group_serving, stop_group_serving,
};
use crate::components::GpuSelector;
use crate::models::config::GpuId;
use crate::models::group::{GroupId, ModelGroup};
use crate::models::serving::{ServingContainer, ServingStatus};
use crate::routes::timer;
#[cfg(target_arch = "wasm32")]
use dioxus::document::eval;
use dioxus::prelude::*;
use std::time::Duration;

/// Props for [`GroupCard`].
#[derive(Props, Clone, PartialEq)]
pub struct GroupCardProps {
    /// The group to display.
    pub group: ModelGroup,
    /// Whether this card is currently selected.
    pub selected: bool,
    /// Called when the card is clicked to select it.
    pub on_select: EventHandler<GroupId>,
    /// Called when the user confirms a rename.
    pub on_rename: EventHandler<(GroupId, String)>,
    /// Called when the "Delete" button is confirmed (release group folder and row only).
    pub on_release: EventHandler<GroupId>,
}

/// A card representing a model group with inline rename and delete action.
#[component]
pub fn GroupCard(props: GroupCardProps) -> Element {
    let mut editing = use_signal(|| false);
    let mut name_buf = use_signal(|| props.group.name.clone());
    let mut confirm_delete = use_signal(|| false);
    let mut copied_path = use_signal(|| false);
    let mut show_start_dialog = use_signal(|| false);
    let mut start_gpu: Signal<Option<GpuId>> = use_signal(|| None);
    let mut serving_busy = use_signal(|| false);
    let mut serving_error: Signal<Option<String>> = use_signal(|| None);
    let mut show_logs = use_signal(|| false);
    let mut serving_tick = use_signal(|| 0u32);
    let mut log_tick = use_signal(|| 0u32);

    let group_id = props.group.id.clone();
    let group_id_release = props.group.id.clone();
    let group_id_for_status = props.group.id.clone();
    let group_id_for_logs = props.group.id.clone();
    let group_id_for_start = props.group.id.clone();
    let group_id_for_stop = props.group.id.clone();

    let serving = use_resource(move || {
        let gid = group_id_for_status.clone();
        let _ = serving_tick();
        async move { get_group_serving_status(gid).await }
    });

    let logs = use_resource(move || {
        let gid = group_id_for_logs.clone();
        let visible = show_logs();
        let _ = log_tick();
        async move {
            if visible {
                get_group_serving_logs(gid, 1_000).await.map(Some)
            } else {
                Ok(None)
            }
        }
    });

    use_future(move || async move {
        loop {
            timer::sleep(Duration::from_secs(2)).await;
            *serving_tick.write() += 1;
            if *show_logs.read() {
                *log_tick.write() += 1;
            }
        }
    });

    let serving_state = current_serving(&serving);
    let serving_status = serving_state.as_ref().map(|s| s.status);
    let is_running = matches!(
        serving_status,
        Some(ServingStatus::Running) | Some(ServingStatus::Starting)
    );

    let member_count = props.group.members.len();
    let models_label = if member_count == 1 {
        "1 model".to_owned()
    } else {
        format!("{member_count} models")
    };
    let dir = props.group.dir_path.to_string_lossy().to_string();
    let dir_for_copy = dir.clone();
    let created = props.group.created_at.format("%b %d, %Y %H:%M").to_string();
    let copy_title = if *copied_path.read() {
        "Copied"
    } else {
        "Copy output path"
    };

    let border_class = if props.selected {
        "glass-card p-5 border-cyan-500 cursor-pointer"
    } else {
        "glass-card p-5 hover:border-slate-600 cursor-pointer"
    };

    rsx! {
        div {
            class: "{border_class}",
            style: "transition: border-color 0.2s ease;",
            onclick: move |_| {
                if !*editing.read() {
                    props.on_select.call(group_id.clone());
                }
            },

            // Header: name or edit input
            div { class: "flex items-center justify-between mb-3",
                if *editing.read() {
                    div {
                        class: "flex items-center gap-2 flex-1",
                        onclick: move |evt| evt.stop_propagation(),
                        input {
                            r#type: "text",
                            class: "field text-sm py-1 flex-1",
                            value: "{name_buf}",
                            oninput: move |evt| name_buf.set(evt.value()),
                            onkeydown: {
                                let gid = props.group.id.clone();
                                move |evt: KeyboardEvent| {
                                    if evt.key() == Key::Enter {
                                        let n = name_buf.read().trim().to_owned();
                                        if !n.is_empty() {
                                            props.on_rename.call((gid.clone(), n));
                                        }
                                        editing.set(false);
                                    } else if evt.key() == Key::Escape {
                                        editing.set(false);
                                    }
                                }
                            },
                        }
                        button {
                            class: "text-xs px-2 py-1 rounded bg-cyan-700 hover:bg-cyan-600 text-white transition-colors",
                            onclick: {
                                let gid = props.group.id.clone();
                                move |evt: MouseEvent| {
                                    evt.stop_propagation();
                                    let n = name_buf.read().trim().to_owned();
                                    if !n.is_empty() {
                                        props.on_rename.call((gid.clone(), n));
                                    }
                                    editing.set(false);
                                }
                            },
                            "Save"
                        }
                        button {
                            class: "text-xs px-2 py-1 rounded bg-slate-700 hover:bg-slate-600 text-slate-300 transition-colors",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                editing.set(false);
                            },
                            "Cancel"
                        }
                    }
                } else {
                    div { class: "flex items-center gap-2 flex-1 min-w-0",
                        h3 { class: "text-slate-100 font-semibold text-sm truncate",
                            "{props.group.name}"
                        }
                        button {
                            class: "flex-shrink-0 text-slate-500 hover:text-cyan-400 transition-colors text-xs",
                            title: "Rename group",
                            onclick: move |evt| {
                                evt.stop_propagation();
                                name_buf.set(props.group.name.clone());
                                editing.set(true);
                            },
                            "✎"
                        }
                    }
                }
            }

            // Stats
            div { class: "flex flex-col gap-1 mb-4",
                div { class: "flex items-center gap-1.5",
                    span {
                        class: "px-2 py-0.5 rounded-full text-xs font-medium bg-slate-700 text-slate-300",
                        "{models_label}"
                    }
                }
                div { class: "flex items-center gap-1.5 min-w-0",
                    p { class: "text-slate-500 text-xs font-mono truncate flex-1 min-w-0", title: "{dir}", "{dir}" }
                    button {
                        class: "flex-shrink-0 w-6 h-6 rounded-md text-slate-500 hover:text-cyan-300 hover:bg-slate-800/70 transition-colors text-xs",
                        title: "{copy_title}",
                        onclick: move |evt| {
                            evt.stop_propagation();
                            copied_path.set(true);
                            let path = dir_for_copy.clone();
                            spawn(async move {
                                copy_output_path(&path).await;
                            });
                        },
                        "⧉"
                    }
                }
                p { class: "text-slate-600 text-xs", "{created}" }
            }

            // Serving status pill
            {serving_status_row(serving_status)}

            // Actions
            div { class: "flex items-center gap-2",
                onclick: move |evt| evt.stop_propagation(),

                if is_running {
                    button {
                        class: "flex-shrink-0 w-8 h-8 rounded-md text-rose-300 hover:text-rose-200 hover:bg-rose-950/40 border border-rose-900/40 transition-colors text-xs disabled:opacity-50",
                        title: "Stop tritonserver",
                        disabled: *serving_busy.read(),
                        onclick: move |_| {
                            let gid = group_id_for_stop.clone();
                            serving_busy.set(true);
                            serving_error.set(None);
                            spawn(async move {
                                if let Err(e) = stop_group_serving(gid).await {
                                    serving_error.set(Some(e.to_string()));
                                }
                                *serving_tick.write() += 1;
                                serving_busy.set(false);
                            });
                        },
                        "■"
                    }
                } else {
                    button {
                        class: "flex-shrink-0 w-8 h-8 rounded-md text-emerald-300 hover:text-emerald-200 hover:bg-emerald-950/40 border border-emerald-900/40 transition-colors text-xs disabled:opacity-50",
                        title: "Start tritonserver",
                        disabled: *serving_busy.read(),
                        onclick: move |_| {
                            show_start_dialog.toggle();
                            serving_error.set(None);
                        },
                        "▶"
                    }
                }
                button {
                    r#type: "button",
                    class: "flex-shrink-0 w-8 h-8 rounded-md text-slate-300 hover:text-cyan-300 hover:bg-slate-800/70 border border-slate-700 transition-colors text-xs",
                    title: if *show_logs.read() { "Hide logs" } else { "Show logs" },
                    onclick: move |_| {
                        show_logs.toggle();
                        *log_tick.write() += 1;
                    },
                    if *show_logs.read() { "▴" } else { "▾" }
                }

                if *confirm_delete.read() {
                    button {
                        class: "flex-1 py-1.5 rounded-lg text-xs font-medium bg-rose-700 hover:bg-rose-600 text-white border border-rose-600 transition-all duration-200",
                        onclick: move |_| {
                            confirm_delete.set(false);
                            props.on_release.call(group_id_release.clone());
                        },
                        "Confirm?"
                    }
                } else {
                    button {
                        class: "flex-1 py-1.5 rounded-lg text-xs font-medium bg-rose-900/40 hover:bg-rose-800/60 text-rose-300 border border-rose-800/50 transition-all duration-200",
                        onclick: move |_| confirm_delete.set(true),
                        "Delete"
                    }
                }
            }

            // Start dialog (inline)
            if *show_start_dialog.read() && !is_running {
                div {
                    class: "mt-3 rounded-lg border border-emerald-900/50 bg-slate-950/70 p-3",
                    onclick: move |evt| evt.stop_propagation(),
                    p { class: "text-xs text-slate-400 mb-2",
                        "Pick a GPU for tritonserver. Image: matching tritonserver tag."
                    }
                    GpuSelector {
                        on_select: move |g| start_gpu.set(g),
                        selected_gpu: *start_gpu.read(),
                    }
                    div { class: "flex justify-end gap-2 mt-3",
                        button {
                            r#type: "button",
                            class: "px-3 py-1.5 rounded-md text-xs text-slate-300 bg-slate-800 hover:bg-slate-700 border border-slate-700 transition-colors",
                            onclick: move |_| show_start_dialog.set(false),
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "px-3 py-1.5 rounded-md text-xs font-semibold text-white bg-emerald-700 hover:bg-emerald-600 transition-colors disabled:opacity-50",
                            disabled: *serving_busy.read() || start_gpu.read().is_none(),
                            onclick: move |_| {
                                let Some(gpu) = *start_gpu.read() else { return; };
                                let gid = group_id_for_start.clone();
                                serving_busy.set(true);
                                serving_error.set(None);
                                show_start_dialog.set(false);
                                spawn(async move {
                                    if let Err(e) = start_group_serving(gid, gpu.0).await {
                                        serving_error.set(Some(e.to_string()));
                                    }
                                    *serving_tick.write() += 1;
                                    serving_busy.set(false);
                                });
                            },
                            "Start"
                        }
                    }
                }
            }

            if let Some(ref err) = *serving_error.read() {
                div { class: "mt-3 rounded-lg px-3 py-2 text-rose-400 text-xs border border-rose-800/50 bg-rose-950/30",
                    "{err}"
                }
            }

            // Logs panel
            if *show_logs.read() {
                div {
                    class: "mt-3 rounded-lg border border-slate-800 bg-slate-950/60 overflow-hidden",
                    onclick: move |evt| evt.stop_propagation(),
                    {match &*logs.read() {
                        None => rsx! {
                            div { class: "px-3 py-3 text-xs text-slate-500", "Loading logs..." }
                        },
                        Some(Err(e)) => rsx! {
                            div { class: "px-3 py-3 text-xs text-rose-400", "Failed to load logs: {e}" }
                        },
                        Some(Ok(None)) => rsx! {
                            div { class: "px-3 py-3 text-xs text-slate-500", "Logs panel closed." }
                        },
                        Some(Ok(Some(text))) => rsx! {
                            pre { class: "max-h-64 overflow-auto p-3 text-[11px] text-slate-300 whitespace-pre-wrap font-mono",
                                if text.trim().is_empty() {
                                    "No tritonserver logs yet."
                                } else {
                                    "{text}"
                                }
                            }
                        },
                    }}
                }
            }
        }
    }
}

fn current_serving(
    serving: &Resource<Result<Option<ServingContainer>, ServerFnError>>,
) -> Option<ServingContainer> {
    serving
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|inner| inner.clone())
}

fn serving_status_row(status: Option<ServingStatus>) -> Element {
    let (label, classes) = match status {
        None | Some(ServingStatus::Stopped) => (
            "stopped",
            "px-2 py-0.5 rounded-full text-[10px] font-semibold bg-slate-800 text-slate-400 border border-slate-700",
        ),
        Some(ServingStatus::Starting) => (
            "starting",
            "px-2 py-0.5 rounded-full text-[10px] font-semibold bg-amber-900/40 text-amber-300 border border-amber-700/50",
        ),
        Some(ServingStatus::Running) => (
            "running",
            "px-2 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-900/40 text-emerald-300 border border-emerald-700/50",
        ),
        Some(ServingStatus::Error) => (
            "error",
            "px-2 py-0.5 rounded-full text-[10px] font-semibold bg-rose-900/40 text-rose-300 border border-rose-700/50",
        ),
    };
    rsx! {
        div { class: "flex items-center mb-3",
            span { class: "{classes}", "{label}" }
        }
    }
}

async fn copy_output_path(path: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let Ok(path_json) = serde_json::to_string(path) else {
            return;
        };
        let js = format!(
            "if (navigator.clipboard) {{
                navigator.clipboard.writeText({path_json});
             }}"
        );
        let _ = eval(&js).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = path;
    }
}
