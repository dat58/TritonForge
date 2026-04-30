//! About page with project overview, author details, and source links.

use dioxus::prelude::*;

const FEATURES: [(&str, &str); 4] = [
    (
        "ONNX to TensorRT",
        "Build TensorRT engines from ONNX models through a focused web workflow.",
    ),
    (
        "Docker-based conversion",
        "Run conversion jobs inside selected TensorRT Docker images with GPU targeting.",
    ),
    (
        "Observable jobs",
        "Track lifecycle status, progress, metadata, and container logs for each run.",
    ),
    (
        "Model organization",
        "Group completed conversion outputs for deployment experiments and comparisons.",
    ),
];

const TECHNOLOGIES: [&str; 8] = [
    "Rust 2024",
    "Dioxus fullstack",
    "Tokio",
    "SQLx",
    "SQLite",
    "TailwindCSS",
    "Docker",
    "Bollard",
];

/// About page that describes TritonForge and project ownership.
#[component]
pub fn AboutPage() -> Element {
    rsx! {
        div { class: "min-h-screen",
            div { class: "max-w-5xl mx-auto px-4 sm:px-6 py-14",
                header { class: "mb-10",
                    p { class: "text-cyan-300 text-xs font-semibold uppercase tracking-wider mb-3",
                        "About TritonForge"
                    }
                    h1 { class: "text-3xl sm:text-4xl font-bold text-slate-100 tracking-tight mb-4",
                        "A focused workflow for ONNX to TensorRT conversion"
                    }
                    p { class: "text-slate-400 text-base sm:text-lg max-w-3xl leading-7",
                        "TritonForge is a fullstack Rust web application built to convert ONNX deep learning models into TensorRT engines. The project makes GPU inference optimization easier to run, track, and repeat through a clean web interface instead of a manual command-line workflow."
                    }
                }

                div { class: "grid grid-cols-1 lg:grid-cols-3 gap-5 mb-5",
                    section { class: "glass-card p-7 lg:col-span-2",
                        h2 { class: "text-xl font-semibold text-slate-100 mb-4", "Why It Exists" }
                        div { class: "flex flex-col gap-4 text-slate-400 text-sm leading-7",
                            p {
                                "TritonForge is designed for developers and machine learning engineers who need a practical way to build TensorRT engines from ONNX models. Instead of manually running trtexec, selecting Docker images, choosing GPUs, configuring shape options, and watching terminal logs, users can manage the full conversion flow from one application."
                            }
                            p {
                                "A conversion starts by uploading an ONNX model or selecting an ONNX file from a server path. The user then chooses a TensorRT Docker image, selects the target GPU, configures TensorRT options, and submits a conversion job. Each job runs inside a Docker container through Bollard, the Rust Docker API client."
                            }
                        }
                    }

                    section { class: "glass-card p-7",
                        h2 { class: "text-xl font-semibold text-slate-100 mb-4", "Author" }
                        div { class: "flex flex-col gap-4",
                            {contact_row("Name", "Dat Vo", None)}
                            {contact_row("Email", "vtdat58@gmail.com", Some("mailto:vtdat58@gmail.com"))}
                            {contact_row("GitHub", "dat58/TritonForge", Some("https://github.com/dat58/TritonForge"))}
                            {contact_row("License", "MIT", None)}
                        }
                    }
                }

                section { class: "glass-card p-7 mb-5",
                    h2 { class: "text-xl font-semibold text-slate-100 mb-4", "How It Works" }
                    p { class: "text-slate-400 text-sm leading-7 mb-5",
                        "The application tracks each job through a clear lifecycle: pending, preparing, converting, finalizing, completed, or failed. During conversion, TritonForge records progress, container logs, model metadata, and job history so users can inspect the result and download the converted model once the process is complete."
                    }
                    p { class: "text-slate-400 text-sm leading-7",
                        "TritonForge also includes model grouping support, making it easier to organize completed conversion outputs for deployment experiments, version comparisons, or inference workflows. The goal is to turn TensorRT engine building into a repeatable, observable, and developer-friendly process."
                    }
                }

                section { class: "grid grid-cols-1 lg:grid-cols-2 gap-5",
                    div { class: "glass-card p-7",
                        h2 { class: "text-xl font-semibold text-slate-100 mb-5", "Key Features" }
                        div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                            for (title, description) in FEATURES {
                                div { class: "rounded-xl border border-slate-800 bg-slate-950/30 p-4",
                                    h3 { class: "text-sm font-semibold text-slate-100 mb-2", "{title}" }
                                    p { class: "text-xs leading-6 text-slate-500", "{description}" }
                                }
                            }
                        }
                    }

                    div { class: "glass-card p-7",
                        h2 { class: "text-xl font-semibold text-slate-100 mb-5", "Technology Stack" }
                        div { class: "flex flex-wrap gap-2",
                            for technology in TECHNOLOGIES {
                                span {
                                    class: "rounded-full border border-cyan-900/70 bg-cyan-950/20 px-3 py-1.5 text-xs font-medium text-cyan-200",
                                    "{technology}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn contact_row(label: &'static str, value: &'static str, href: Option<&'static str>) -> Element {
    rsx! {
        div {
            p { class: "text-xs font-semibold uppercase tracking-wider text-slate-500 mb-1",
                "{label}"
            }
            if let Some(url) = href {
                a {
                    href: url,
                    target: if url.starts_with("http") { "_blank" } else { "_self" },
                    rel: "noopener noreferrer",
                    class: "text-sm font-medium text-cyan-300 hover:text-cyan-200 transition-colors break-all",
                    "{value}"
                }
            } else {
                p { class: "text-sm font-medium text-slate-200", "{value}" }
            }
        }
    }
}
