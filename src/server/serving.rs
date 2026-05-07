//! Tritonserver orchestration: launches and tears down a container that serves
//! a model group, persisting its state and streaming its logs into SQLite.

use crate::errors::AppError;
use crate::models::config::GpuId;
use crate::models::group::{GroupId, ModelGroup};
use crate::models::serving::{ServingContainer, ServingPortBindings, ServingStatus};
use crate::server::db::{self, DbPool, NewJobLog};
use crate::server::docker::DockerService;
use bollard::container::LogOutput;
use bollard::models::{ContainerCreateBody, DeviceRequest, HostConfig, PortBinding};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, LogsOptionsBuilder, RemoveContainerOptionsBuilder,
    StopContainerOptionsBuilder,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_stream::StreamExt;
use tracing::{Instrument, instrument};

/// Container ports Triton listens on (HTTP, gRPC, metrics).
const TRITON_HTTP_PORT: u16 = 8000;
const TRITON_GRPC_PORT: u16 = 8001;
const TRITON_METRICS_PORT: u16 = 8002;

const SERVE_LOG_BATCH_SIZE: usize = 25;
const SERVE_LOG_FLUSH_INTERVAL: Duration = Duration::from_secs(1);

/// Derives the matching tritonserver image tag from a TensorRT image tag.
///
/// `nvcr.io/nvidia/tensorrt:24.08-py3` → `nvcr.io/nvidia/tritonserver:24.08-py3`.
/// Returns `None` when the input doesn't match the expected `nvcr.io` format.
pub fn triton_image_for_tensorrt(tensorrt_tag: &str) -> Option<String> {
    let (repo, version) = tensorrt_tag.rsplit_once(':')?;
    if !repo.contains("tensorrt") {
        return None;
    }
    let triton_repo = repo.replace("tensorrt", "tritonserver");
    Some(format!("{triton_repo}:{version}"))
}

/// Builds the container name used for a group's tritonserver.
pub fn container_name_for_group(group_id: &GroupId) -> String {
    format!("tritonforge-serve-{group_id}")
}

/// Creates and starts a tritonserver container that bind-mounts the group
/// directory as `/models`. Returns the started container record (status = `Running`).
#[instrument(skip(docker, group), fields(group_id = %group.id, image_tag, gpu_id = %gpu_id))]
pub async fn start_tritonserver(
    docker: &DockerService,
    group: &ModelGroup,
    image_tag: &str,
    gpu_id: GpuId,
    ports: ServingPortBindings,
    network: &str,
) -> Result<ServingContainer, AppError> {
    if group.members.is_empty() {
        return Err(AppError::Validation(
            "cannot start tritonserver for an empty group".into(),
        ));
    }

    let mount_source = ensure_absolute_path(&group.dir_path)?;
    let container_name = container_name_for_group(&group.id);

    // Best-effort cleanup of any lingering container with the same name from a prior run.
    let _ = docker
        .client()
        .remove_container(
            &container_name,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await;

    let port_bindings = build_port_bindings(ports);
    let exposed_ports = build_exposed_ports();
    let mut labels = HashMap::new();
    labels.insert("tritonforge.group_id".to_string(), group.id.to_string());

    let host_config = HostConfig {
        binds: Some(vec![format!("{mount_source}:/models:ro")]),
        port_bindings: Some(port_bindings),
        network_mode: Some(network.to_owned()),
        device_requests: Some(vec![DeviceRequest {
            driver: Some(String::new()),
            count: None,
            device_ids: Some(vec![gpu_id.to_string()]),
            capabilities: Some(vec![vec!["gpu".to_string()]]),
            options: Some(HashMap::new()),
        }]),
        auto_remove: Some(false),
        ..Default::default()
    };

    let body = ContainerCreateBody {
        image: Some(image_tag.to_owned()),
        cmd: Some(vec![
            "tritonserver".to_string(),
            "--model-repository=/models".to_string(),
        ]),
        labels: Some(labels),
        exposed_ports: Some(exposed_ports),
        host_config: Some(host_config),
        ..Default::default()
    };

    let response = docker
        .client()
        .create_container(
            Some(
                CreateContainerOptionsBuilder::default()
                    .name(&container_name)
                    .build(),
            ),
            body,
        )
        .await?;
    let container_id = response.id;
    tracing::info!(container_id = %container_id, "tritonserver container created");

    docker
        .client()
        .start_container(
            &container_id,
            None::<bollard::query_parameters::StartContainerOptions>,
        )
        .await?;
    tracing::info!(container_id = %container_id, "tritonserver container started");

    Ok(ServingContainer {
        group_id: group.id.clone(),
        container_id,
        container_name,
        image_tag: image_tag.to_owned(),
        gpu_id: gpu_id.0,
        status: ServingStatus::Running,
        error_message: None,
        started_at: chrono::Utc::now(),
        stopped_at: None,
    })
}

/// Stops and removes a tritonserver container. Idempotent if the container
/// has already been removed.
#[instrument(skip(docker), fields(container_id))]
pub async fn stop_tritonserver(docker: &DockerService, container_id: &str) -> Result<(), AppError> {
    match docker
        .client()
        .stop_container(
            container_id,
            Some(StopContainerOptionsBuilder::default().t(10).build()),
        )
        .await
    {
        Ok(())
        | Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => {}
        Err(e) => return Err(AppError::Docker(e)),
    }

    match docker
        .client()
        .remove_container(
            container_id,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await
    {
        Ok(())
        | Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => Ok(()),
        Err(e) => Err(AppError::Docker(e)),
    }
}

/// Spawns a background task that follows container logs and persists batches
/// into `tritonserver_logs`. Carries the parent span so logs stay correlated.
pub fn spawn_log_pump(
    docker: DockerService,
    pool: Arc<DbPool>,
    group_id: GroupId,
    container_id: String,
    span: tracing::Span,
) {
    tokio::spawn(
        async move {
            stream_serving_logs(&docker, &pool, &group_id, &container_id).await;
        }
        .instrument(span),
    );
}

async fn stream_serving_logs(
    docker: &DockerService,
    pool: &DbPool,
    group_id: &GroupId,
    container_id: &str,
) {
    let options = LogsOptionsBuilder::default()
        .follow(true)
        .stdout(true)
        .stderr(true)
        .build();

    let mut stream = docker.client().logs(container_id, Some(options));
    let mut batch = ServingLogBatch::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(output) => {
                push_log_output(&mut batch, container_id, pool, output).await;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "tritonserver log stream error");
                break;
            }
        }
    }

    batch.flush(pool, container_id).await;

    // Container exited — mark stopped (best effort; user-driven Stop wins).
    if let Err(e) = db::update_serving_status(pool, group_id, ServingStatus::Stopped, None).await {
        tracing::warn!(error = ?e, "failed to mark serving container stopped after stream end");
    }
}

async fn push_log_output(
    batch: &mut ServingLogBatch,
    container_id: &str,
    pool: &DbPool,
    output: LogOutput,
) {
    let stream = match output {
        LogOutput::StdErr { .. } => "stderr",
        LogOutput::StdOut { .. } => "stdout",
        LogOutput::StdIn { .. } => "stdin",
        LogOutput::Console { .. } => "console",
    };
    let text = output.to_string();

    for line in text.lines() {
        batch.push(NewJobLog::new(stream, line));
        if batch.should_flush() {
            batch.flush(pool, container_id).await;
        }
    }
}

fn build_port_bindings(ports: ServingPortBindings) -> HashMap<String, Option<Vec<PortBinding>>> {
    let mut bindings = HashMap::new();
    for (container_port, host_port) in [
        (TRITON_HTTP_PORT, ports.http),
        (TRITON_GRPC_PORT, ports.grpc),
        (TRITON_METRICS_PORT, ports.metrics),
    ] {
        bindings.insert(
            format!("{container_port}/tcp"),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(host_port.to_string()),
            }]),
        );
    }
    bindings
}

fn build_exposed_ports() -> Vec<String> {
    [TRITON_HTTP_PORT, TRITON_GRPC_PORT, TRITON_METRICS_PORT]
        .into_iter()
        .map(|port| format!("{port}/tcp"))
        .collect()
}

fn ensure_absolute_path(path: &Path) -> Result<String, AppError> {
    let canonical = std::fs::canonicalize(path).map_err(|e| {
        AppError::Validation(format!(
            "group directory '{}' is not accessible: {e}",
            path.display()
        ))
    })?;
    Ok(canonical.to_string_lossy().into_owned())
}

struct ServingLogBatch {
    logs: Vec<NewJobLog>,
    last_flush: Instant,
}

impl ServingLogBatch {
    fn new() -> Self {
        Self {
            logs: Vec::with_capacity(SERVE_LOG_BATCH_SIZE),
            last_flush: Instant::now(),
        }
    }

    fn push(&mut self, log: NewJobLog) {
        self.logs.push(log);
    }

    fn should_flush(&self) -> bool {
        self.logs.len() >= SERVE_LOG_BATCH_SIZE
            || self.last_flush.elapsed() >= SERVE_LOG_FLUSH_INTERVAL
    }

    async fn flush(&mut self, pool: &DbPool, container_id: &str) {
        if self.logs.is_empty() {
            return;
        }
        if let Err(e) = db::append_serving_logs_batch(pool, container_id, &self.logs).await {
            tracing::warn!(
                error = ?e,
                count = self.logs.len(),
                "failed to persist tritonserver logs"
            );
        }
        self.logs.clear();
        self.last_flush = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triton_image_swap_keeps_version_suffix() {
        assert_eq!(
            triton_image_for_tensorrt("nvcr.io/nvidia/tensorrt:24.08-py3").as_deref(),
            Some("nvcr.io/nvidia/tritonserver:24.08-py3")
        );
        assert_eq!(
            triton_image_for_tensorrt("nvcr.io/nvidia/tensorrt:23.04-py3").as_deref(),
            Some("nvcr.io/nvidia/tritonserver:23.04-py3")
        );
    }

    #[test]
    fn triton_image_returns_none_for_non_tensorrt_tag() {
        assert!(triton_image_for_tensorrt("ubuntu:22.04").is_none());
        assert!(triton_image_for_tensorrt("nvcr.io/nvidia/tritonserver:24.08-py3").is_none());
    }

    #[test]
    fn container_name_uses_group_id() {
        let id = GroupId::new();
        assert_eq!(
            container_name_for_group(&id),
            format!("tritonforge-serve-{id}")
        );
    }

    #[test]
    fn build_port_bindings_uses_selected_host_ports() {
        let bindings = build_port_bindings(ServingPortBindings {
            http: 9000,
            grpc: 9001,
            metrics: 9002,
        });

        assert_eq!(
            bindings["8000/tcp"]
                .as_ref()
                .and_then(|bindings| bindings.first())
                .and_then(|binding| binding.host_port.as_deref()),
            Some("9000")
        );
        assert_eq!(
            bindings["8001/tcp"]
                .as_ref()
                .and_then(|bindings| bindings.first())
                .and_then(|binding| binding.host_port.as_deref()),
            Some("9001")
        );
        assert_eq!(
            bindings["8002/tcp"]
                .as_ref()
                .and_then(|bindings| bindings.first())
                .and_then(|binding| binding.host_port.as_deref()),
            Some("9002")
        );
    }
}
