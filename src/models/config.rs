//! Application configuration and GPU/image descriptor types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Opaque GPU device index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GpuId(pub u32);

impl std::fmt::Display for GpuId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Metadata for a detected NVIDIA GPU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// Device index as reported by nvidia-smi.
    pub id: GpuId,
    /// Human-readable GPU model name.
    pub name: String,
    /// Total GPU memory in megabytes.
    pub memory_mb: u64,
}

/// A supported TensorRT Docker image entry from the images config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorRtImage {
    /// Display name shown in the UI.
    pub name: String,
    /// Full Docker image tag (e.g. `nvcr.io/nvidia/tensorrt:24.08-py3`).
    pub tag: String,
    /// CUDA toolkit version bundled in this image.
    pub cuda_version: String,
    /// TensorRT library version bundled in this image.
    pub tensorrt_version: String,
}

/// Runtime configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Directory where uploaded model files are staged.
    pub upload_dir: PathBuf,
    /// Directory where completed TRT engine files are stored.
    pub output_dir: PathBuf,
    /// Maximum allowed upload size in megabytes.
    pub max_upload_size_mb: u64,
    /// Maximum seconds a single conversion job may run before timing out.
    pub conversion_timeout_secs: u64,
    /// Path to the Docker daemon socket.
    pub docker_socket: PathBuf,
}

impl AppConfig {
    /// Loads configuration from environment variables, falling back to safe defaults.
    pub fn from_env() -> Self {
        Self {
            upload_dir: PathBuf::from(
                std::env::var("UPLOAD_DIR")
                    .unwrap_or_else(|_| "/tmp/tensorrt-converter/uploads".into()),
            ),
            output_dir: PathBuf::from(
                std::env::var("OUTPUT_DIR").unwrap_or_else(|_| "/data/tensorrt-models".into()),
            ),
            max_upload_size_mb: std::env::var("MAX_UPLOAD_SIZE_MB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2048),
            conversion_timeout_secs: std::env::var("CONVERSION_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1800),
            docker_socket: PathBuf::from(
                std::env::var("DOCKER_SOCKET").unwrap_or_else(|_| "/var/run/docker.sock".into()),
            ),
        }
    }
}
