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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionJob {
    /// Unique job identifier.
    pub id: JobId,
    /// Human-readable model name used for output directory naming.
    pub model_name: String,
    /// Triton model version directory name.
    pub model_version: u32,
    /// Source model format.
    pub model_format: ModelFormat,
    /// Docker image tag for the TensorRT container.
    pub image_tag: String,
    /// GPU device index to run conversion on.
    pub gpu_id: GpuId,
    /// TensorRT conversion options.
    pub trt_options: TrtOptions,
    /// Current lifecycle state.
    pub status: JobStatus,
    /// Conversion progress from 0 to 100.
    pub progress_percent: u8,
    /// Path to the completed Triton model directory, if available.
    pub output_path: Option<PathBuf>,
    /// Human-readable error description when status is `Failed`.
    pub error_message: Option<String>,
    /// Timestamp when the job was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last status update.
    pub updated_at: DateTime<Utc>,
}

/// One persisted stdout/stderr line emitted by a conversion container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionJobLog {
    /// Monotonic row identifier for stable ordering.
    pub id: i64,
    /// Conversion job that produced the log line.
    pub job_id: JobId,
    /// Container stream name, usually `stdout` or `stderr`.
    pub stream: String,
    /// Log line text without Docker multiplexing metadata.
    pub message: String,
    /// Timestamp when the log row was persisted.
    pub created_at: DateTime<Utc>,
}

/// TensorRT conversion options for trtexec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrtOptions {
    /// Use explicit batch dimension (recommended for ONNX).
    pub explicit_batch: bool,
    /// Minimum shapes for dynamic axes (e.g., "input:1x3x224x224").
    pub min_shapes: Option<String>,
    /// Optimal shapes for dynamic axes.
    pub opt_shapes: Option<String>,
    /// Maximum shapes for dynamic axes.
    pub max_shapes: Option<String>,
    /// Workspace size in MiB.
    pub workspace_mb: u32,
    /// Number of minimization iterations.
    pub min_timing: u32,
    /// Number of averaging iterations.
    pub avg_timing: u32,
    /// Enable FP16 precision.
    pub fp16: bool,
}

impl Default for TrtOptions {
    fn default() -> Self {
        Self {
            explicit_batch: true,
            min_shapes: None,
            opt_shapes: None,
            max_shapes: None,
            workspace_mb: 4096,
            min_timing: 8,
            avg_timing: 16,
            fp16: true,
        }
    }
}

/// Request payload for submitting a new conversion job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmitJobRequest {
    /// Source used to stage the ONNX model before conversion.
    pub input_source: ModelInputSource,
    /// Human-readable model name used for output directory naming.
    pub model_name: String,
    /// Triton model version directory name.
    pub model_version: u32,
    /// Docker image tag for the TensorRT container.
    pub image_tag: String,
    /// GPU device index to run conversion on.
    pub gpu_id: u32,
    /// TensorRT conversion options.
    pub trt_options: TrtOptions,
}

/// Model source selected on the upload page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelInputSource {
    /// Browser-uploaded ONNX bytes sent with the job request.
    UploadedFile,
    /// Existing ONNX file path readable by the server process.
    ServerPath {
        /// Path to an ONNX model on the server filesystem.
        path: String,
    },
}

// Import GpuId here to avoid circular dependency in the struct definition
use crate::models::config::GpuId;

impl std::fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Onnx => write!(f, "onnx"),
        }
    }
}

impl std::str::FromStr for ModelFormat {
    type Err = crate::errors::AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "onnx" => Ok(Self::Onnx),
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
