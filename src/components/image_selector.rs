//! TensorRT Docker image picker dropdown component.

use crate::api::get_available_images;
use dioxus::prelude::*;

/// Dropdown for selecting a locally available TensorRT Docker image.
///
/// Calls `get_available_images` on mount and displays image name, TensorRT version, and CUDA version.
#[component]
pub fn ImageSelector(
    on_select: EventHandler<Option<String>>,
    selected_image: Option<String>,
) -> Element {
    let images = use_resource(get_available_images);

    rsx! {
        div { class: "flex flex-col gap-1",
            label { class: "text-sm font-medium text-gray-300", "TensorRT Docker Image" }
            {match &*images.read() {
                None => rsx! {
                    div {
                        class: "bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-gray-400 text-sm animate-pulse",
                        "Loading images..."
                    }
                },
                Some(Err(e)) => rsx! {
                    div {
                        class: "bg-red-900/20 border border-red-700 rounded-lg px-3 py-2 text-red-400 text-sm",
                        "Failed to load images: {e}"
                    }
                },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    div {
                        class: "bg-yellow-900/20 border border-yellow-700 rounded-lg px-3 py-2 text-yellow-400 text-sm",
                        "No TensorRT images found locally. Run "
                        code { class: "font-mono text-yellow-300", "docker pull nvcr.io/nvidia/tensorrt:..." }
                        " first."
                    }
                },
                Some(Ok(list)) => rsx! {
                    select {
                        class: "bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-gray-100 text-sm focus:outline-none focus:border-blue-500 cursor-pointer w-full",
                        onchange: move |evt| {
                            let val = evt.value();
                            let selected = if val.is_empty() { None } else { Some(val) };
                            on_select.call(selected);
                        },
                        option { value: "", "— Select Image —" }
                        for img in list {
                            option {
                                value: "{img.tag}",
                                selected: selected_image.as_deref() == Some(img.tag.as_str()),
                                "{img.name}  (TRT {img.tensorrt_version} / CUDA {img.cuda_version})"
                            }
                        }
                    }
                },
            }}
        }
    }
}
