//! config.pbtxt template picker dropdown component.

use crate::api::get_available_templates;
use crate::models::job::ModelFormat;
use crate::models::template::ConfigTemplate;
use dioxus::prelude::*;

/// Dropdown for selecting a Triton `config.pbtxt` template.
#[component]
pub fn TemplateSelector(
    on_select: EventHandler<Option<String>>,
    selected_template: Option<String>,
    model_format: Option<ModelFormat>,
) -> Element {
    let mut templates: Signal<Option<Result<Vec<ConfigTemplate>, String>>> = use_signal(|| None);

    // use_effect is client-only (skipped during SSR), keeping the initial render tree
    // identical on both server and client so hydration assigns data-dioxus-id correctly.
    use_effect(move || {
        spawn(async move {
            let result = get_available_templates().await.map_err(|e| e.to_string());
            templates.set(Some(result));
        });
    });

    rsx! {
        div { class: "flex flex-col gap-1.5",
            label { class: "text-xs font-semibold uppercase tracking-wider text-slate-400",
                "Config Template"
            }
            {match &*templates.read() {
                None => rsx! {
                    div { class: "field text-slate-500 animate-pulse", "Loading templates..." }
                },
                Some(Err(e)) => rsx! {
                    div { class: "rounded-lg px-3 py-2.5 text-rose-400 text-sm border border-rose-800/50 bg-rose-950/30",
                        "Failed to load templates: {e}"
                    }
                },
                Some(Ok(list)) => {
                    let filtered: Vec<_> = list
                        .iter()
                        .filter(|t| {
                            model_format
                                .as_ref()
                                .is_none_or(|fmt| t.compatible_formats.contains(fmt))
                        })
                        .collect();

                    if filtered.is_empty() {
                        rsx! {
                            div { class: "rounded-lg px-3 py-2.5 text-amber-400 text-sm border border-amber-800/50 bg-amber-950/30",
                                "No templates available. Add .pbtxt files to templates/ directory."
                            }
                        }
                    } else {
                        rsx! {
                            select {
                                class: "field",
                                onchange: move |evt| {
                                    let val = evt.value();
                                    let selected = if val.is_empty() { None } else { Some(val) };
                                    on_select.call(selected);
                                },
                                option { value: "", "— Select Template —" }
                                for tmpl in &filtered {
                                    option {
                                        value: "{tmpl.name}",
                                        selected: selected_template.as_deref()
                                            == Some(tmpl.name.as_str()),
                                        "{tmpl.description}"
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
