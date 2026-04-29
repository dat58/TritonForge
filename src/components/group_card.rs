//! Card component for a model group with inline rename and action buttons.

use crate::models::group::{GroupId, ModelGroup};
use dioxus::prelude::*;

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
    /// Called when the "Release" button is clicked (delete folder only).
    pub on_release: EventHandler<GroupId>,
    /// Called when the "Delete" button is confirmed (delete folder + source files).
    pub on_delete: EventHandler<GroupId>,
}

/// A card representing a model group with inline rename and release/delete actions.
#[component]
pub fn GroupCard(props: GroupCardProps) -> Element {
    let mut editing = use_signal(|| false);
    let mut name_buf = use_signal(|| props.group.name.clone());
    let mut confirm_delete = use_signal(|| false);

    let group_id = props.group.id.clone();
    let group_id_release = props.group.id.clone();
    let group_id_delete = props.group.id.clone();

    let member_count = props.group.members.len();
    let models_label = if member_count == 1 {
        "1 model".to_owned()
    } else {
        format!("{member_count} models")
    };
    let dir = props.group.dir_path.to_string_lossy().to_string();
    let created = props.group.created_at.format("%b %d, %Y").to_string();

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
                p { class: "text-slate-500 text-xs font-mono truncate", title: "{dir}", "{dir}" }
                p { class: "text-slate-600 text-xs", "{created}" }
            }

            // Actions
            div { class: "flex gap-2",
                onclick: move |evt| evt.stop_propagation(),

                button {
                    class: "flex-1 py-1.5 rounded-lg text-xs font-medium bg-amber-900/40 hover:bg-amber-800/60 text-amber-300 border border-amber-800/50 transition-all duration-200",
                    onclick: move |_| props.on_release.call(group_id_release.clone()),
                    "Release"
                }

                if *confirm_delete.read() {
                    button {
                        class: "flex-1 py-1.5 rounded-lg text-xs font-medium bg-rose-700 hover:bg-rose-600 text-white border border-rose-600 transition-all duration-200",
                        onclick: move |_| {
                            confirm_delete.set(false);
                            props.on_delete.call(group_id_delete.clone());
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
        }
    }
}
