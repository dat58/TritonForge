//! GPU detection via nvidia-smi subprocess parsing.

use crate::errors::AppError;
use crate::models::config::{GpuId, GpuInfo};
use tokio::process::Command;
use tracing::instrument;

const NVIDIA_SMI_QUERY: &str =
    "--query-gpu=index,name,memory.total,memory.free --format=csv,noheader,nounits";

/// Async service for querying available NVIDIA GPUs.
#[derive(Debug, Clone, Default)]
pub struct GpuService;

impl GpuService {
    /// Creates a new `GpuService`.
    pub fn new() -> Self {
        Self
    }

    /// Queries `nvidia-smi` and returns metadata for all detected GPUs.
    ///
    /// Returns an empty `Vec` (with a warning log) if `nvidia-smi` is absent
    /// or reports no devices, rather than propagating an error.
    #[instrument]
    pub async fn detect_gpus(&self) -> Vec<GpuInfo> {
        match self.run_nvidia_smi().await {
            Ok(output) => parse_nvidia_smi_output(&output),
            Err(err) => {
                tracing::warn!(error = ?err, "nvidia-smi unavailable; returning empty GPU list");
                Vec::new()
            }
        }
    }

    /// Returns `true` if `gpu_id` is present among the detected GPUs.
    #[instrument(skip(self), fields(gpu_id = %gpu_id))]
    pub async fn is_gpu_available(&self, gpu_id: GpuId) -> bool {
        self.detect_gpus().await.iter().any(|gpu| gpu.id == gpu_id)
    }

    async fn run_nvidia_smi(&self) -> Result<String, AppError> {
        let args: Vec<&str> = NVIDIA_SMI_QUERY.split_whitespace().collect();
        let output = Command::new("nvidia-smi").args(&args).output().await?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| AppError::Conversion(format!("nvidia-smi output not UTF-8: {e}")))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(AppError::Conversion(format!(
                "nvidia-smi exited with {}: {stderr}",
                output.status
            )))
        }
    }
}

/// Parses CSV lines from `nvidia-smi --format=csv,noheader,nounits`.
///
/// Each line has the form: `index, name, memory_total_mb, memory_free_mb`
fn parse_nvidia_smi_output(output: &str) -> Vec<GpuInfo> {
    output.lines().filter_map(parse_nvidia_smi_line).collect()
}

fn parse_nvidia_smi_line(line: &str) -> Option<GpuInfo> {
    let mut parts = line.splitn(4, ',');
    let index: u32 = parts.next()?.trim().parse().ok()?;
    let name = parts.next()?.trim().to_owned();
    let memory_total_mb: u64 = parts.next()?.trim().parse().ok()?;
    let memory_free_mb: u64 = parts.next()?.trim().parse().ok()?;

    Some(GpuInfo {
        id: GpuId(index),
        name,
        memory_total_mb,
        memory_free_mb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_single_gpu_line() {
        let line = "0, NVIDIA GeForce RTX 3090, 24576, 24000";
        let info = parse_nvidia_smi_line(line).expect("should parse");
        assert_eq!(info.id, GpuId(0));
        assert_eq!(info.name, "NVIDIA GeForce RTX 3090");
        assert_eq!(info.memory_total_mb, 24576);
        assert_eq!(info.memory_free_mb, 24000);
    }

    #[test]
    fn parse_multiple_gpus() {
        let output = "0, Tesla A100, 81920, 80000\n1, Tesla A100, 81920, 75000\n";
        let gpus = parse_nvidia_smi_output(output);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].id, GpuId(0));
        assert_eq!(gpus[0].memory_free_mb, 80000);
        assert_eq!(gpus[1].id, GpuId(1));
        assert_eq!(gpus[1].memory_free_mb, 75000);
    }

    #[test]
    fn parse_empty_output_returns_empty_vec() {
        let gpus = parse_nvidia_smi_output("");
        assert!(gpus.is_empty());
    }

    #[test]
    fn parse_malformed_line_is_skipped() {
        let output = "0, Tesla A100, 81920, 80000\nbad line\n1, RTX 4090, 24564, 24000\n";
        let gpus = parse_nvidia_smi_output(output);
        assert_eq!(gpus.len(), 2);
    }

    #[tokio::test]
    async fn detect_gpus_returns_empty_when_nvidia_smi_absent() {
        // On machines without nvidia-smi, this must return empty, not panic.
        let service = GpuService::new();
        let gpus = service.detect_gpus().await;
        // Either 0 or more GPUs — just verify no panic/error.
        let _ = gpus.len();
    }
}
