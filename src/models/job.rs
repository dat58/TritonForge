//! Job domain types: identifiers, status, format, and the full job record.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Opaque identifier for a conversion job.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    /// Creates a new random `JobId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Supported input model formats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFormat {
    /// ONNX open neural network exchange format.
    Onnx,
    /// TensorFlow SavedModel directory format.
    TensorFlowSavedModel,
}

/// Lifecycle states for a conversion job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job created, awaiting resources.
    Pending,
    /// Preparing workspace and validating inputs.
    Preparing,
    /// Actively running trtexec inside the Docker container.
    Converting,
    /// Moving output files and generating config.pbtxt.
    Finalizing,
    /// Conversion succeeded; output is available.
    Completed,
    /// Conversion failed; see `error_message` for details.
    Failed,
}

/// Full record for a model conversion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionJob {
    /// Unique job identifier.
    pub id: JobId,
    /// Human-readable model name used for output directory naming.
    pub model_name: String,
    /// Source model format.
    pub model_format: ModelFormat,
    /// Docker image tag for the TensorRT container.
    pub image_tag: String,
    /// GPU device index to run conversion on.
    pub gpu_id: GpuId,
    /// Name of the config.pbtxt template to apply.
    pub template_name: String,
    /// Current lifecycle state.
    pub status: JobStatus,
    /// Conversion progress from 0 to 100.
    pub progress_percent: u8,
    /// Path to the completed TRT engine file, if available.
    pub output_path: Option<PathBuf>,
    /// Human-readable error description when status is `Failed`.
    pub error_message: Option<String>,
    /// Timestamp when the job was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last status update.
    pub updated_at: DateTime<Utc>,
}

// Import GpuId here to avoid circular dependency in the struct definition
use crate::models::config::GpuId;

impl std::fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Onnx => write!(f, "onnx"),
            Self::TensorFlowSavedModel => write!(f, "tensorflow_saved_model"),
        }
    }
}

impl std::str::FromStr for ModelFormat {
    type Err = crate::errors::AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "onnx" => Ok(Self::Onnx),
            "tensorflow_saved_model" => Ok(Self::TensorFlowSavedModel),
            other => Err(crate::errors::AppError::Validation(format!(
                "unknown model format: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Preparing => write!(f, "preparing"),
            Self::Converting => write!(f, "converting"),
            Self::Finalizing => write!(f, "finalizing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = crate::errors::AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "preparing" => Ok(Self::Preparing),
            "converting" => Ok(Self::Converting),
            "finalizing" => Ok(Self::Finalizing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(crate::errors::AppError::Validation(format!(
                "unknown job status: {other}"
            ))),
        }
    }
}
