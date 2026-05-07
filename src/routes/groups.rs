//! Groups management page — create, view, and manage model groups.

use crate::api::{
    add_models_to_group, create_model_group, get_group_serving_logs, list_completed_jobs,
    list_model_groups, release_model_group, remove_model_from_group, rename_model_group,
    start_group_serving,
};
use crate::app::Route;
use crate::components::{GpuSelector, GroupCard, ServingView};
use crate::models::config::GpuId;
use crate::models::group::{GroupId, ModelGroup, ModelGroupMember};
use crate::models::job::ConversionJob;
use crate::routes::timer;
use dioxus::prelude::*;
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

/// Model groups management page.
#[component]
pub fn GroupsPage() -> Element {
    let mut selected_group_id: Signal<Option<GroupId>> = use_signal(|| None);
    let mut checked_models: Signal<HashSet<String>> = use_signal(HashSet::new);
    let mut refresh_tick = use_signal(|| 0u32);
    let grouping_busy = use_signal(|| false);
    let mut create_busy = use_signal(|| false);
    let mut serving_view: Signal<ServingView> = use_signal(|| ServingView::None);
    let mut start_gpu: Signal<Option<GpuId>> = use_signal(|| None);
    let serving_panel_busy = use_signal(|| false);
    let mut serving_panel_error: Signal<Option<String>> = use_signal(|| None);
    let mut log_tick = use_signal(|| 0u32);

    let groups = use_resource(move || {
        let _ = refresh_tick();
        async move { list_model_groups().await }
    });

    let completed = use_resource(|| async move { list_completed_jobs().await });

    let serving_logs = use_resource(move || {
        let view = serving_view();
        let _ = log_tick();
        async move {
            match view {
                ServingView::Logs(gid) => get_group_serving_logs(gid, 1_000).await.map(Some),
                _ => Ok(None),
            }
        }
    });

    use_future(move || async move {
        loop {
            timer::sleep(Duration::from_secs(2)).await;

            if matches!(&*serving_view.read(), ServingView::Logs(_)) {
                *log_tick.write() += 1;
            }
        }
    });

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
                        // Cards grid
                        div { class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4",
                            for group in list {
                                {
                                    let gid = group.id.clone();
                                    let gid_release = group.id.clone();
                                    let gid_release_check = group.id.clone();
                                    let member_keys = group_member_keys(group);
                                    let selected = selected_group_id.read().as_ref() == Some(&gid);
                                    let current_serving_view = serving_view.read().clone();
                                    rsx! {
                                        GroupCard {
                                            group: group.clone(),
                                            selected,
                                            serving_view: current_serving_view,
                                            on_select: move |id: GroupId| {
                                                // Toggle: clicking the selected card collapses it.
                                                if selected_group_id.read().as_ref() == Some(&id) {
                                                    selected_group_id.set(None);
                                                    checked_models.write().clear();
                                                } else {
                                                    selected_group_id.set(Some(id));
                                                    checked_models.set(member_keys.clone());
                                                }
                                            },
                                            on_rename: move |(id, name): (GroupId, String)| {
                                                spawn(async move {
                                                    let _ = rename_model_group(id, name).await;
                                                    *refresh_tick.write() += 1;
                                                });
                                            },
                                            on_release: move |_: GroupId| {
                                                let id = gid_release.clone();
                                                let check = gid_release_check.clone();
                                                close_serving_view_for(serving_view, &check);
                                                spawn(async move {
                                                    let _ = release_model_group(id).await;
                                                    *refresh_tick.write() += 1;
                                                    if selected_group_id.read().as_ref() == Some(&check) {
                                                        selected_group_id.set(None);
                                                        checked_models.write().clear();
                                                    }
                                                });
                                            },
                                            on_request_start: move |id: GroupId| {
                                                let already_open = matches!(
                                                    &*serving_view.read(),
                                                    ServingView::StartDialog(open_id) if open_id == &id
                                                );

                                                if already_open {
                                                    serving_view.set(ServingView::None);
                                                } else {
                                                    start_gpu.set(None);
                                                    serving_panel_error.set(None);
                                                    serving_view.set(ServingView::StartDialog(id));
                                                }
                                            },
                                            on_toggle_logs: move |id: GroupId| {
                                                let already_open = matches!(
                                                    &*serving_view.read(),
                                                    ServingView::Logs(open_id) if open_id == &id
                                                );

                                                if already_open {
                                                    serving_view.set(ServingView::None);
                                                } else {
                                                    serving_panel_error.set(None);
                                                    serving_view.set(ServingView::Logs(id));
                                                    *log_tick.write() += 1;
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }

                        {serving_panel(ServingPanelState {
                            groups: list,
                            serving_view,
                            start_gpu,
                            serving_busy: serving_panel_busy,
                            serving_error: serving_panel_error,
                            refresh_tick,
                            log_tick,
                            logs: &serving_logs,
                        })}

                        // Model picker — shown only when a group card is selected.
                        {
                            let maybe_group = selected_group_id
                                .read()
                                .as_ref()
                                .and_then(|id| list.iter().find(|g| &g.id == id))
                                .cloned();

                            if let Some(sel_group) = maybe_group {
                                model_picker(sel_group, &completed, checked_models, grouping_busy, refresh_tick)
                            } else {
                                rsx! {}
                            }
                        }
                    },
                }}
            }
        }
    }
}

fn close_serving_view_for(mut serving_view: Signal<ServingView>, group_id: &GroupId) {
    let targets_group = matches!(
        &*serving_view.read(),
        ServingView::StartDialog(open_id) | ServingView::Logs(open_id) if open_id == group_id
    );

    if targets_group {
        serving_view.set(ServingView::None);
    }
}

struct ServingPanelState<'a> {
    groups: &'a [ModelGroup],
    serving_view: Signal<ServingView>,
    start_gpu: Signal<Option<GpuId>>,
    serving_busy: Signal<bool>,
    serving_error: Signal<Option<String>>,
    refresh_tick: Signal<u32>,
    log_tick: Signal<u32>,
    logs: &'a Resource<Result<Option<String>, ServerFnError>>,
}

fn serving_panel(state: ServingPanelState<'_>) -> Element {
    let ServingPanelState {
        groups,
        mut serving_view,
        mut start_gpu,
        mut serving_busy,
        mut serving_error,
        mut refresh_tick,
        mut log_tick,
        logs,
    } = state;

    let view = serving_view.read().clone();

    match view {
        ServingView::None => rsx! {},
        ServingView::StartDialog(gid) => {
            let Some(group) = groups.iter().find(|group| group.id == gid) else {
                return rsx! {};
            };
            let group_name = group.name.clone();
            let gid_for_start = gid.clone();

            rsx! {
                div {
                    class: "mt-6 rounded-xl border border-emerald-900/50 bg-slate-950/70 p-5",
                    h2 { class: "text-sm font-semibold text-slate-200 mb-1",
                        "Start tritonserver for \"{group_name}\""
                    }
                    p { class: "text-xs text-slate-500 mb-4",
                        "Pick a GPU for tritonserver. Image: matching tritonserver tag."
                    }
                    GpuSelector {
                        on_select: move |g| start_gpu.set(g),
                        selected_gpu: *start_gpu.read(),
                    }
                    if let Some(ref err) = *serving_error.read() {
                        div { class: "mt-4 rounded-lg px-3 py-2 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30",
                            "{err}"
                        }
                    }
                    div { class: "flex justify-end gap-2 mt-4",
                        button {
                            r#type: "button",
                            class: "px-4 py-2 rounded-lg text-sm text-slate-300 bg-slate-800 hover:bg-slate-700 border border-slate-700 transition-colors disabled:opacity-50",
                            disabled: *serving_busy.read(),
                            onclick: move |_| {
                                serving_error.set(None);
                                serving_view.set(ServingView::None);
                            },
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "px-4 py-2 rounded-lg text-sm font-semibold text-white bg-emerald-700 hover:bg-emerald-600 transition-colors disabled:opacity-50",
                            disabled: *serving_busy.read() || start_gpu.read().is_none(),
                            onclick: move |_| {
                                let Some(gpu) = *start_gpu.read() else { return; };
                                let gid = gid_for_start.clone();
                                serving_busy.set(true);
                                serving_error.set(None);
                                spawn(async move {
                                    match start_group_serving(gid.clone(), gpu.0).await {
                                        Ok(()) => {
                                            serving_view.set(ServingView::Logs(gid));
                                            *log_tick.write() += 1;
                                        }
                                        Err(e) => serving_error.set(Some(e.to_string())),
                                    }
                                    *refresh_tick.write() += 1;
                                    serving_busy.set(false);
                                });
                            },
                            if *serving_busy.read() {
                                span { class: "inline-block w-3 h-3 rounded-full border-2 border-white border-t-transparent animate-spin mr-1.5" }
                            }
                            "Start"
                        }
                    }
                }
            }
        }
        ServingView::Logs(gid) => {
            let group_name = groups
                .iter()
                .find(|group| group.id == gid)
                .map(|group| group.name.clone())
                .unwrap_or_else(|| gid.to_string());

            rsx! {
                div { class: "mt-6 rounded-xl border border-slate-800 bg-slate-950/70 overflow-hidden",
                    div { class: "flex items-center justify-between px-5 py-3 border-b border-slate-800 bg-slate-900/50",
                        h2 { class: "text-sm font-semibold text-slate-200",
                            "tritonserver logs for \"{group_name}\""
                        }
                        button {
                            r#type: "button",
                            class: "w-8 h-8 inline-flex items-center justify-center rounded-md text-slate-300 hover:text-cyan-300 hover:bg-slate-800/70 border border-slate-700 transition-colors text-xs",
                            title: "Close logs",
                            onclick: move |_| serving_view.set(ServingView::None),
                            "×"
                        }
                    }
                    {match &*logs.read() {
                        None => rsx! {
                            div { class: "px-5 py-4 text-sm text-slate-500", "Loading logs..." }
                        },
                        Some(Err(e)) => rsx! {
                            div { class: "px-5 py-4 text-sm text-rose-400", "Failed to load logs: {e}" }
                        },
                        Some(Ok(None)) => rsx! {
                            div { class: "px-5 py-4 text-sm text-slate-500", "Open logs to load tritonserver output." }
                        },
                        Some(Ok(Some(text))) => rsx! {
                            pre { class: "block h-[76vh] min-h-[36rem] max-h-[76vh] max-w-full overflow-x-auto overflow-y-scroll overscroll-contain p-4 text-xs text-slate-300 whitespace-pre font-mono",
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

fn group_member_keys(group: &ModelGroup) -> HashSet<String> {
    group
        .members
        .iter()
        .map(|member| model_key(&member.job_id, &member.model_name))
        .collect()
}

fn model_key(job_id: &str, model_name: &str) -> String {
    format!("{job_id}/{model_name}")
}

fn short_tensorrt_image_tag(image_tag: &str) -> &str {
    let tag = image_tag.rsplit_once(':').map_or(image_tag, |(_, tag)| tag);

    tag.strip_suffix("-py3").unwrap_or(tag)
}

#[derive(Debug, Clone, PartialEq)]
struct CompletedJobGroup {
    image_tag: String,
    jobs: Vec<ConversionJob>,
}

fn group_completed_jobs_by_image_tag(jobs: &[ConversionJob]) -> Vec<CompletedJobGroup> {
    let mut groups: BTreeMap<String, Vec<ConversionJob>> = BTreeMap::new();

    for job in jobs {
        groups
            .entry(job.image_tag.clone())
            .or_default()
            .push(job.clone());
    }

    groups
        .into_iter()
        .rev()
        .map(|(image_tag, jobs)| CompletedJobGroup { image_tag, jobs })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{group_completed_jobs_by_image_tag, short_tensorrt_image_tag};
    use crate::models::config::GpuId;
    use crate::models::job::{ConversionJob, JobId, JobStatus, ModelFormat, TrtOptions};
    use chrono::Utc;
    use std::path::PathBuf;

    fn sample_job(model_name: &str, image_tag: &str) -> ConversionJob {
        ConversionJob {
            id: JobId::new(),
            model_name: model_name.to_owned(),
            model_version: 1,
            model_format: ModelFormat::Onnx,
            image_tag: image_tag.to_owned(),
            gpu_id: GpuId(0),
            trt_options: TrtOptions::default(),
            status: JobStatus::Completed,
            progress_percent: 100,
            output_path: Some(PathBuf::from("/tmp/model")),
            error_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn short_tensorrt_image_tag_drops_repository_and_py_suffix() {
        assert_eq!(
            short_tensorrt_image_tag("nvcr.io/nvidia/tensorrt:24.04-py3"),
            "24.04"
        );
        assert_eq!(
            short_tensorrt_image_tag("nvcr.io/nvidia/tensorrt:23.09-py3"),
            "23.09"
        );
    }

    #[test]
    fn short_tensorrt_image_tag_keeps_non_py_tag() {
        assert_eq!(
            short_tensorrt_image_tag("tensorrt:24.04-custom"),
            "24.04-custom"
        );
    }

    #[test]
    fn group_completed_jobs_by_image_tag_keeps_same_image_together() {
        let jobs = vec![
            sample_job("resnet50", "nvcr.io/nvidia/tensorrt:24.04-py3"),
            sample_job("yolov8", "nvcr.io/nvidia/tensorrt:24.08-py3"),
            sample_job("bert", "nvcr.io/nvidia/tensorrt:24.04-py3"),
        ];

        let groups = group_completed_jobs_by_image_tag(&jobs);
        let trt_2404 = groups
            .iter()
            .find(|group| group.image_tag == "nvcr.io/nvidia/tensorrt:24.04-py3")
            .expect("24.04 group exists");

        assert_eq!(trt_2404.jobs.len(), 2);
        assert!(trt_2404.jobs.iter().any(|job| job.model_name == "resnet50"));
        assert!(trt_2404.jobs.iter().any(|job| job.model_name == "bert"));
    }

    #[test]
    fn group_completed_jobs_by_image_tag_sorts_groups_by_tag_descending() {
        let jobs = vec![
            sample_job("resnet50", "nvcr.io/nvidia/tensorrt:24.04-py3"),
            sample_job("yolov8", "nvcr.io/nvidia/tensorrt:24.08-py3"),
            sample_job("bert", "nvcr.io/nvidia/tensorrt:23.12-py3"),
        ];

        let image_tags: Vec<_> = group_completed_jobs_by_image_tag(&jobs)
            .into_iter()
            .map(|group| group.image_tag)
            .collect();

        assert_eq!(
            image_tags,
            vec![
                "nvcr.io/nvidia/tensorrt:24.08-py3",
                "nvcr.io/nvidia/tensorrt:24.04-py3",
                "nvcr.io/nvidia/tensorrt:23.12-py3",
            ]
        );
    }
}

fn selected_additions(
    desired: &HashSet<String>,
    current: &HashSet<String>,
) -> Vec<ModelGroupMember> {
    desired
        .difference(current)
        .filter_map(|key| {
            let (job_id, model_name) = key.split_once('/')?;
            Some(ModelGroupMember {
                job_id: job_id.to_owned(),
                model_name: model_name.to_owned(),
            })
        })
        .collect()
}

fn selected_removals(current: &HashSet<String>, desired: &HashSet<String>) -> Vec<String> {
    current
        .difference(desired)
        .filter_map(|key| {
            key.split_once('/')
                .map(|(_, model_name)| model_name.to_owned())
        })
        .collect()
}

async fn update_group_models(
    group_id: GroupId,
    additions: Vec<ModelGroupMember>,
    removals: Vec<String>,
) -> Result<(), ServerFnError> {
    for model_name in removals {
        remove_model_from_group(group_id.clone(), model_name).await?;
    }

    if !additions.is_empty() {
        add_models_to_group(group_id, additions).await?;
    }

    Ok(())
}

/// Dropdown panel showing all completed models in a grid; ticked if already in the group.
fn model_picker(
    group: ModelGroup,
    completed: &Resource<Result<Vec<ConversionJob>, ServerFnError>>,
    checked_models: Signal<HashSet<String>>,
    mut grouping_busy: Signal<bool>,
    mut refresh_tick: Signal<u32>,
) -> Element {
    let current_keys = group_member_keys(&group);
    let desired_keys = checked_models.read().clone();
    let additions_count = desired_keys.difference(&current_keys).count();
    let removals_count = current_keys.difference(&desired_keys).count();
    let change_count = additions_count + removals_count;
    let has_changes = change_count > 0;

    let button_label = if *grouping_busy.read() {
        "Updating...".to_owned()
    } else if has_changes {
        format!("Update Models ({change_count})")
    } else {
        "Update Models".to_owned()
    };

    let group_name = group.name.clone();
    let group_id = group.id.clone();
    let current_keys_for_update = current_keys.clone();

    rsx! {
        div { class: "mt-8 rounded-xl border border-slate-700/60 bg-slate-900/40 overflow-hidden",

            // Panel header
            div { class: "flex items-center justify-between px-5 py-3 border-b border-slate-700/60 bg-slate-800/40",
                div { class: "flex items-center gap-2",
                    span { class: "text-slate-400 text-sm", "▾" }
                    h2 { class: "text-sm font-semibold text-slate-200",
                        "Add models to \"{group_name}\""
                    }
                }
                button {
                    class: "px-4 py-1.5 rounded-lg text-sm font-medium text-white transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed",
                    style: "background: linear-gradient(135deg, #0891b2, #0d9488);",
                    disabled: !has_changes || *grouping_busy.read(),
                    onclick: move |_| {
                        let gid = group_id.clone();
                        let desired = checked_models.read().clone();
                        let additions = selected_additions(&desired, &current_keys_for_update);
                        let removals = selected_removals(&current_keys_for_update, &desired);
                        grouping_busy.set(true);
                        spawn(async move {
                            let _ = update_group_models(gid, additions, removals).await;
                            grouping_busy.set(false);
                            *refresh_tick.write() += 1;
                        });
                    },
                    if *grouping_busy.read() {
                        span { class: "inline-block w-3 h-3 rounded-full border-2 border-white border-t-transparent animate-spin mr-1.5" }
                    }
                    "{button_label}"
                }
            }

            // Model grid
            div { class: "p-4",
                {match &*completed.read() {
                    None => rsx! {
                        div { class: "flex items-center gap-3 text-slate-400 py-6",
                            div { class: "w-4 h-4 rounded-full border-2 border-cyan-500 border-t-transparent animate-spin" }
                            "Loading models..."
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "rounded-lg p-4 text-rose-400 border border-rose-800/50 bg-rose-950/20 text-sm",
                            "Failed to load models: {e}"
                        }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        div { class: "text-slate-500 text-sm py-6 text-center",
                            "No completed models found. "
                            Link {
                                to: Route::Home {},
                                class: "text-cyan-400 hover:text-cyan-300 transition-colors",
                                "Convert a model first →"
                            }
                        }
                    },
                    Some(Ok(list)) => {
                        let grouped_jobs = group_completed_jobs_by_image_tag(list);

                        rsx! {
                            div { class: "flex flex-col gap-5",
                                for group in grouped_jobs {
                                    {
                                        let short_tag = short_tensorrt_image_tag(&group.image_tag).to_owned();
                                        let full_tag = group.image_tag.clone();
                                        let count = group.jobs.len();

                                        rsx! {
                                            section { key: "{full_tag}", class: "rounded-xl border border-slate-800/70 bg-slate-950/20 p-4",
                                                div { class: "flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between mb-3",
                                                    div {
                                                        h3 { class: "text-sm font-semibold text-slate-200",
                                                            "TensorRT {short_tag}"
                                                        }
                                                        p { class: "text-xs text-slate-600 break-all",
                                                            "{full_tag}"
                                                        }
                                                    }
                                                    span { class: "text-xs text-slate-500",
                                                        "{count} models"
                                                    }
                                                }

                                                div { class: "grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3",
                                                    for job in group.jobs {
                                                        {model_picker_card(job, &current_keys, &desired_keys, checked_models)}
                                                    }
                                                }
                                            }
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
}

fn model_picker_card(
    job: ConversionJob,
    current_keys: &HashSet<String>,
    desired_keys: &HashSet<String>,
    mut checked_models: Signal<HashSet<String>>,
) -> Element {
    let key = model_key(&job.id.to_string(), &job.model_name);
    let key_toggle = key.clone();
    let in_group = current_keys.contains(&key);
    let selected = desired_keys.contains(&key);
    let created = job.created_at.format("%b %d").to_string();
    let image_tag = short_tensorrt_image_tag(&job.image_tag).to_owned();

    let (card_border, card_bg, indicator_style, model_text) = model_card_style(in_group, selected);

    rsx! {
        div {
            key: "{key}",
            class: "relative rounded-lg border p-4 pr-8 cursor-pointer transition-all duration-150 {card_border} {card_bg}",
            onclick: move |_| {
                let mut models = checked_models.write();
                if models.contains(&key_toggle) {
                    models.remove(&key_toggle);
                } else {
                    models.insert(key_toggle.clone());
                }
            },

            div {
                class: "w-4 h-4 rounded border flex items-center justify-center flex-shrink-0",
                style: "position:absolute;top:0.625rem;right:0.625rem;z-index:1;{indicator_style}",
                if selected {
                    span { class: "text-white text-[10px] leading-none font-bold", "✓" }
                }
            }

            p {
                class: "text-sm font-medium truncate {model_text}",
                "{job.model_name}"
            }
            p { class: "text-xs text-slate-500 mt-0.5",
                "{job.model_format} · v{job.model_version}"
            }
            p { class: "text-xs text-slate-500 mt-0.5",
                "TensorRT · {image_tag}"
            }
            p { class: "text-xs text-slate-600 mt-1", "{created}" }
        }
    }
}

fn model_card_style(
    in_group: bool,
    selected: bool,
) -> (&'static str, &'static str, &'static str, &'static str) {
    if in_group && selected {
        (
            "border-emerald-800/50",
            "bg-emerald-950/20",
            "background:#065f46;border-color:#065f46;",
            "text-emerald-300",
        )
    } else if in_group {
        (
            "border-amber-700/70",
            "bg-amber-950/20",
            "border-color:#d97706;",
            "text-amber-200",
        )
    } else if selected {
        (
            "border-cyan-600",
            "bg-cyan-950/20",
            "background:#0891b2;border-color:#0891b2;",
            "text-cyan-200",
        )
    } else {
        (
            "border-slate-700/60",
            "hover:bg-slate-800/40",
            "border-color:#475569;",
            "text-slate-200",
        )
    }
}
