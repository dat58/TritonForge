//! Docker-based TensorRT conversion pipeline.

use crate::errors::AppError;
use crate::models::config::AppConfig;
use crate::models::job::{ConversionJob, JobId, JobStatus, ModelFormat};
use crate::server::db::{self, DbPool};
use crate::server::docker::DockerService;
use crate::server::storage::StorageService;
use bollard::models::{ContainerCreateBody, DeviceRequest, HostConfig};
use bollard::query_parameters::{LogsOptionsBuilder, RemoveContainerOptionsBuilder};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{Instrument, instrument};

/// 16 GiB memory limit per conversion container.
const CONTAINER_MEMORY_BYTES: i64 = 16 * 1024 * 1024 * 1024;

/// Paths inside the conversion container.
const CONTAINER_INPUT_DIR: &str = "/input";
const CONTAINER_OUTPUT_DIR: &str = "/output";

/// Progress percentage assigned at each trtexec output milestone.
const PROGRESS_MILESTONES: &[(&str, u8)] = &[
    ("Building engine", 20),
    ("building optimization", 40),
    ("Finished engine building", 70),
    ("Serializing engine", 80),
    ("Inference averag", 90),
];

/// Orchestrates the full Docker-based model conversion lifecycle.
pub struct ConversionService {
    docker: DockerService,
    storage: StorageService,
    pool: Arc<DbPool>,
    timeout: Duration,
}

impl ConversionService {
    /// Creates a new `ConversionService`.
    pub fn new(
        docker: DockerService,
        storage: StorageService,
        pool: Arc<DbPool>,
        config: &AppConfig,
    ) -> Self {
        Self {
            docker,
            storage,
            pool,
            timeout: Duration::from_secs(config.conversion_timeout_secs),
        }
    }

    /// Executes the full conversion pipeline for `job` in the current async task.
    ///
    /// Call this inside `tokio::spawn` for background execution.
    #[instrument(skip(self), fields(job_id = %job.id, image = %job.image_tag, gpu_id = %job.gpu_id))]
    pub async fn run_conversion(
        &self,
        job: ConversionJob,
        model_path: PathBuf,
    ) -> Result<PathBuf, AppError> {
        let span = tracing::Span::current();

        // Build path strings for bind mounts and container command.
        let model_filename = model_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("model");
        let container_model_path = format!("{CONTAINER_INPUT_DIR}/{model_filename}");
        let container_name = format!("tritonforge-{}", job.id);

        // Create a temp directory for container output.
        let temp_output_dir = self.storage.upload_dir().join(format!("out-{}", job.id));
        tokio::fs::create_dir_all(&temp_output_dir).await?;

        let result = self
            .run_pipeline(
                &job,
                &model_path,
                &temp_output_dir,
                &container_model_path,
                &container_name,
                span,
            )
            .await;

        // Best-effort cleanup of the uploaded model temp file.
        let _ = self.storage.cleanup_temp(&model_path).await;

        result
    }

    async fn run_pipeline(
        &self,
        job: &ConversionJob,
        model_path: &Path,
        temp_output_dir: &Path,
        container_model_path: &str,
        container_name: &str,
        span: tracing::Span,
    ) -> Result<PathBuf, AppError> {
        db::update_job_status(&self.pool, &job.id, JobStatus::Preparing, 0).await?;

        let binds = vec![
            format!("{}:{container_model_path}:ro", model_path.display()),
            format!("{}:{CONTAINER_OUTPUT_DIR}", temp_output_dir.display()),
        ];

        let cmd = build_trtexec_cmd(&job.model_format, container_model_path);
        let container_id = self
            .create_and_start_container(job, container_name, binds, cmd)
            .await?;

        db::update_job_status(&self.pool, &job.id, JobStatus::Converting, 5).await?;

        let exit_code = self.stream_logs_and_wait(job, &container_id, span).await;

        // Remove the container (auto_remove should handle it, but be explicit).
        let _ = self
            .docker
            .client()
            .remove_container(
                &container_id,
                Some(RemoveContainerOptionsBuilder::default().force(true).build()),
            )
            .await;

        match exit_code? {
            0 => self.finalise_job(job, temp_output_dir).await,
            code => {
                let msg = format!("container exited with code {code}");
                db::update_job_failed(&self.pool, &job.id, &msg).await?;
                Err(AppError::Conversion(msg))
            }
        }
    }

    async fn create_and_start_container(
        &self,
        job: &ConversionJob,
        container_name: &str,
        binds: Vec<String>,
        cmd: Vec<String>,
    ) -> Result<String, AppError> {
        let mut labels = HashMap::new();
        labels.insert("tritonforge.job_id".to_string(), job.id.to_string());

        let host_config = HostConfig {
            binds: Some(binds),
            device_requests: Some(vec![DeviceRequest {
                driver: Some(String::new()),
                count: None,
                device_ids: Some(vec![job.gpu_id.to_string()]),
                capabilities: Some(vec![vec!["gpu".to_string()]]),
                options: Some(HashMap::new()),
            }]),
            memory: Some(CONTAINER_MEMORY_BYTES),
            auto_remove: Some(false), // we manage removal explicitly
            ..Default::default()
        };

        use bollard::query_parameters::CreateContainerOptionsBuilder;
        let options = CreateContainerOptionsBuilder::default()
            .name(container_name)
            .build();

        let body = ContainerCreateBody {
            image: Some(job.image_tag.clone()),
            cmd: Some(cmd),
            labels: Some(labels),
            host_config: Some(host_config),
            ..Default::default()
        };

        let response = self
            .docker
            .client()
            .create_container(Some(options), body)
            .await?;

        let id = response.id;
        tracing::info!(container_id = %id, "container created");

        self.docker
            .client()
            .start_container(
                &id,
                None::<bollard::query_parameters::StartContainerOptions>,
            )
            .await?;
        tracing::info!(container_id = %id, "container started");

        Ok(id)
    }

    async fn stream_logs_and_wait(
        &self,
        job: &ConversionJob,
        container_id: &str,
        span: tracing::Span,
    ) -> Result<i64, AppError> {
        let (progress_tx, mut progress_rx) = mpsc::channel::<u8>(16);

        // Spawn log streaming in a separate task.
        let pool_clone = Arc::clone(&self.pool);
        let job_id = job.id.clone();
        let docker_clone = self.docker.clone();
        let cid = container_id.to_owned();

        let log_task = tokio::spawn(
            async move {
                stream_container_logs(&docker_clone, &cid, &job_id, &pool_clone, progress_tx).await;
            }
            .instrument(span.clone()),
        );

        // Wait for container exit with timeout.
        let wait_result = tokio::time::timeout(
            self.timeout,
            self.docker
                .client()
                .wait_container(
                    container_id,
                    None::<bollard::query_parameters::WaitContainerOptions>,
                )
                .next(),
        )
        .await;

        log_task.abort();
        // Drain remaining progress updates.
        while progress_rx.try_recv().is_ok() {}

        match wait_result {
            Ok(Some(Ok(response))) => Ok(response.status_code),
            Ok(Some(Err(e))) => Err(e.into()),
            Ok(None) => Ok(0),
            Err(_elapsed) => {
                let msg = format!(
                    "conversion timed out after {} seconds",
                    self.timeout.as_secs()
                );
                db::update_job_failed(&self.pool, &job.id, &msg).await?;
                Err(AppError::Conversion(msg))
            }
        }
    }

    async fn finalise_job(
        &self,
        job: &ConversionJob,
        temp_output_dir: &Path,
    ) -> Result<PathBuf, AppError> {
        db::update_job_status(&self.pool, &job.id, JobStatus::Finalizing, 95).await?;

        let engine_path = find_engine_file(temp_output_dir).await?;
        let final_path = self
            .storage
            .move_to_output(&engine_path, &job.id, &job.model_name)
            .await?;

        db::update_job_completed(&self.pool, &job.id, &final_path).await?;
        Ok(final_path)
    }
}

async fn stream_container_logs(
    docker: &DockerService,
    container_id: &str,
    job_id: &JobId,
    pool: &DbPool,
    progress_tx: mpsc::Sender<u8>,
) {
    let options = LogsOptionsBuilder::default()
        .follow(true)
        .stdout(true)
        .stderr(true)
        .build();

    let mut stream = docker.client().logs(container_id, Some(options));

    while let Some(result) = stream.next().await {
        match result {
            Ok(output) => {
                let line = output.to_string();
                tracing::trace!(line = %line.trim(), "container log");

                if let Some(progress) = parse_progress(&line) {
                    let _ =
                        db::update_job_status(pool, job_id, JobStatus::Converting, progress).await;
                    let _ = progress_tx.try_send(progress);
                }
            }
            Err(e) => {
                tracing::warn!(error = ?e, "log stream error");
                break;
            }
        }
    }
}

fn parse_progress(line: &str) -> Option<u8> {
    PROGRESS_MILESTONES
        .iter()
        .find(|(pattern, _)| line.contains(pattern))
        .map(|(_, pct)| *pct)
}

fn build_trtexec_cmd(format: &ModelFormat, container_model_path: &str) -> Vec<String> {
    let input_flag = match format {
        ModelFormat::Onnx => format!("--onnx={container_model_path}"),
        ModelFormat::TensorFlowSavedModel => {
            format!("--savedModel={container_model_path}")
        }
    };

    vec![
        "trtexec".to_string(),
        input_flag,
        format!("--saveEngine={CONTAINER_OUTPUT_DIR}/model.engine"),
        "--fp16".to_string(),
    ]
}

async fn find_engine_file(dir: &Path) -> Result<PathBuf, AppError> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("engine") {
            return Ok(path);
        }
    }
    Err(AppError::Conversion(format!(
        "no .engine file found in {}",
        dir.display()
    )))
}
