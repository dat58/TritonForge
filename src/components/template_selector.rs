//! config.pbtxt template picker dropdown component.

use crate::api::get_available_templates;
use crate::models::job::ModelFormat;
use dioxus::prelude::*;

/// Dropdown for selecting a Triton `config.pbtxt` template.
///
/// Filters the list to only show templates compatible with the given `model_format`.
#[component]
pub fn TemplateSelector(
    on_select: EventHandler<Option<String>>,
    selected_template: Option<String>,
    model_format: Option<ModelFormat>,
) -> Element {
    let templates = use_resource(get_available_templates);

    rsx! {
        div { class: "flex flex-col gap-1",
            label { class: "text-sm font-medium text-gray-300", "Config Template" }
            {match &*templates.read() {
                None => rsx! {
                    div {
                        class: "bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-gray-400 text-sm animate-pulse",
                        "Loading templates..."
                    }
                },
                Some(Err(e)) => rsx! {
                    div {
                        class: "bg-red-900/20 border border-red-700 rounded-lg px-3 py-2 text-red-400 text-sm",
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
                            div {
                                class: "bg-yellow-900/20 border border-yellow-700 rounded-lg px-3 py-2 text-yellow-400 text-sm",
                                "No templates available. Add .pbtxt files to the templates/ directory."
                            }
                        }
                    } else {
                        rsx! {
                            select {
                                class: "bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-gray-100 text-sm focus:outline-none focus:border-blue-500 cursor-pointer w-full",
                                onchange: move |evt| {
                                    let val = evt.value();
                                    let selected = if val.is_empty() { None } else { Some(val) };
                                    on_select.call(selected);
                                },
                                option { value: "", "— Select Template —" }
                                for tmpl in &filtered {
                                    option {
                                        value: "{tmpl.name}",
                                        selected: selected_template.as_deref() == Some(tmpl.name.as_str()),
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
