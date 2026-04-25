//! config.pbtxt template descriptors.

use crate::models::job::ModelFormat;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Describes a config.pbtxt template available for selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigTemplate {
    /// Unique template identifier used in API calls.
    pub name: String,
    /// Human-readable description shown in the UI.
    pub description: String,
    /// Absolute path to the template file on disk.
    pub file_path: PathBuf,
    /// Model formats this template is compatible with.
    pub compatible_formats: Vec<ModelFormat>,
}

/// Scans `templates_dir` and returns all `.pbtxt` template descriptors.
///
/// Only available on server builds because it performs filesystem I/O
/// that is not meaningful in the browser context.
#[cfg(not(target_arch = "wasm32"))]
pub async fn load_templates(
    templates_dir: &std::path::Path,
) -> Result<Vec<ConfigTemplate>, crate::errors::AppError> {
    use tokio::fs;

    let mut templates = Vec::new();
    let mut entries = fs::read_dir(templates_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("pbtxt") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_owned();

            templates.push(ConfigTemplate {
                description: name.replace('_', " "),
                name,
                compatible_formats: vec![ModelFormat::Onnx, ModelFormat::TensorFlowSavedModel],
                file_path: path,
            });
        }
    }

    Ok(templates)
}
