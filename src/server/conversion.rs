//! Docker-based TensorRT conversion pipeline.

use crate::errors::AppError;
use crate::models::config::AppConfig;
use crate::models::job::{ConversionJob, JobId, JobStatus, TrtOptions};
use crate::server::db::{self, DbPool};
use crate::server::docker::DockerService;
use crate::server::onnx_config::generate_config_pbtxt;
use crate::server::storage::StorageService;
use bollard::container::LogOutput;
use bollard::models::{ContainerCreateBody, DeviceRequest, HostConfig};
use bollard::query_parameters::{LogsOptionsBuilder, RemoveContainerOptionsBuilder};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_stream::StreamExt;
use tracing::{Instrument, instrument};

/// 16 GiB memory limit per conversion container.
const CONTAINER_MEMORY_BYTES: i64 = 16 * 1024 * 1024 * 1024;

/// Paths inside the conversion container.
const CONTAINER_INPUT_DIR: &str = "/input";
const CONTAINER_OUTPUT_DIR: &str = "/output";

/// Progress percentage assigned at each trtexec output milestone.
const PROGRESS_MILESTONES: &[(&str, u8)] = &[
    ("building engine", 20),
    ("Trace details", 40),
    ("Average on", 60),
    ("Performance summary", 90),
];

const LOG_BATCH_SIZE: usize = 25;
const LOG_FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const LOG_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

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
        if let Err(error) = self.storage.cleanup_temp(&model_path).await {
            tracing::warn!(error = ?error, path = %model_path.display(), "upload cleanup failed");
        }

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

        let cmd = build_trtexec_cmd(container_model_path, &job.trt_options);
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
            0 => self.finalise_job(job, model_path, temp_output_dir).await,
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
        let pool_clone = Arc::clone(&self.pool);
        let job_id = job.id.clone();
        let docker_clone = self.docker.clone();
        let cid = container_id.to_owned();

        let log_task = tokio::spawn(
            async move {
                stream_container_logs(&docker_clone, &cid, &job_id, &pool_clone).await;
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

        if wait_result.is_err() {
            log_task.abort();
        } else {
            drain_log_task(log_task).await;
        }

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
        model_path: &Path,
        temp_output_dir: &Path,
    ) -> Result<PathBuf, AppError> {
        db::update_job_status(&self.pool, &job.id, JobStatus::Finalizing, 95).await?;

        let plan_path = find_plan_file(temp_output_dir).await?;
        let config_pbtxt = generate_config_pbtxt(model_path, &job.model_name).await?;
        let model_dir = self
            .storage
            .move_to_output(
                &plan_path,
                &job.id,
                &job.model_name,
                job.model_version,
                &config_pbtxt,
            )
            .await?;

        db::update_job_completed(&self.pool, &job.id, &model_dir).await?;
        Ok(model_dir)
    }
}

async fn stream_container_logs(
    docker: &DockerService,
    container_id: &str,
    job_id: &JobId,
    pool: &DbPool,
) {
    let options = LogsOptionsBuilder::default()
        .follow(true)
        .stdout(true)
        .stderr(true)
        .build();

    let mut stream = docker.client().logs(container_id, Some(options));
    let mut batch = LogBatch::new();
    let mut progress = ProgressTracker::new(5);

    while let Some(result) = stream.next().await {
        match result {
            Ok(output) => {
                persist_log_output(pool, job_id, &mut batch, &mut progress, output).await;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "log stream error");
                break;
            }
        }
    }

    batch.flush(pool, job_id).await;
}

async fn drain_log_task(mut log_task: tokio::task::JoinHandle<()>) {
    tokio::select! {
        result = &mut log_task => match result {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(error = ?e, "log stream task failed");
            }
        },
        () = tokio::time::sleep(LOG_DRAIN_TIMEOUT) => {
            log_task.abort();
            tracing::warn!(
                timeout_secs = LOG_DRAIN_TIMEOUT.as_secs(),
                "timed out draining container logs"
            );
        }
    }
}

async fn persist_log_output(
    pool: &DbPool,
    job_id: &JobId,
    batch: &mut LogBatch,
    progress: &mut ProgressTracker,
    output: LogOutput,
) {
    let stream_name = log_stream_name(&output);
    let text = output.to_string();

    for line in log_lines(&text) {
        tracing::trace!(stream = stream_name, line = %line.trim(), "container log");
        batch.push(db::NewJobLog::new(stream_name, line));
        update_progress_from_line(pool, job_id, progress, line).await;

        if batch.should_flush() {
            batch.flush(pool, job_id).await;
        }
    }
}

async fn update_progress_from_line(
    pool: &DbPool,
    job_id: &JobId,
    progress: &mut ProgressTracker,
    line: &str,
) {
    let Some(next_progress) = progress.next_update(parse_progress(line)) else {
        return;
    };

    if let Err(e) = db::update_job_status(pool, job_id, JobStatus::Converting, next_progress).await
    {
        tracing::warn!(
            error = ?e,
            progress = next_progress,
            "failed to persist conversion progress"
        );
    }
}

fn log_stream_name(output: &LogOutput) -> &'static str {
    match output {
        LogOutput::StdErr { .. } => "stderr",
        LogOutput::StdOut { .. } => "stdout",
        LogOutput::StdIn { .. } => "stdin",
        LogOutput::Console { .. } => "console",
    }
}

fn log_lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

struct LogBatch {
    logs: Vec<db::NewJobLog>,
    last_flush: Instant,
}

impl LogBatch {
    fn new() -> Self {
        Self {
            logs: Vec::with_capacity(LOG_BATCH_SIZE),
            last_flush: Instant::now(),
        }
    }

    fn push(&mut self, log: db::NewJobLog) {
        self.logs.push(log);
    }

    fn should_flush(&self) -> bool {
        self.logs.len() >= LOG_BATCH_SIZE || self.last_flush.elapsed() >= LOG_FLUSH_INTERVAL
    }

    async fn flush(&mut self, pool: &DbPool, job_id: &JobId) {
        if self.logs.is_empty() {
            return;
        }

        if let Err(e) = db::append_job_logs_batch(pool, job_id, &self.logs).await {
            tracing::warn!(
                error = ?e,
                count = self.logs.len(),
                "failed to persist container logs"
            );
        }

        self.logs.clear();
        self.last_flush = Instant::now();
    }
}

#[derive(Debug)]
struct ProgressTracker {
    last_progress: u8,
}

impl ProgressTracker {
    fn new(initial_progress: u8) -> Self {
        Self {
            last_progress: initial_progress,
        }
    }

    fn next_update(&mut self, candidate: Option<u8>) -> Option<u8> {
        let next_progress = candidate?;
        if next_progress <= self.last_progress {
            return None;
        }

        self.last_progress = next_progress;
        Some(next_progress)
    }
}

fn parse_progress(line: &str) -> Option<u8> {
    PROGRESS_MILESTONES
        .iter()
        .find(|(pattern, _)| line.contains(pattern))
        .map(|(_, pct)| *pct)
}

fn build_trtexec_cmd(container_model_path: &str, options: &TrtOptions) -> Vec<String> {
    let mut cmd = vec!["trtexec".to_string()];

    cmd.push(format!("--onnx={container_model_path}"));
    cmd.push(format!("--saveEngine={CONTAINER_OUTPUT_DIR}/model.plan"));

    if options.explicit_batch {
        cmd.push("--explicitBatch".to_string());
    }

    if let Some(ref min) = options.min_shapes {
        cmd.push(format!("--minShapes={min}"));
    }
    if let Some(ref opt) = options.opt_shapes {
        cmd.push(format!("--optShapes={opt}"));
    }
    if let Some(ref max) = options.max_shapes {
        cmd.push(format!("--maxShapes={max}"));
    }

    cmd.push(format!("--workspace={}", options.workspace_mb));
    cmd.push(format!("--minTiming={}", options.min_timing));
    cmd.push(format!("--avgTiming={}", options.avg_timing));

    if options.fp16 {
        cmd.push("--fp16".to_string());
    }

    cmd
}

async fn find_plan_file(dir: &Path) -> Result<PathBuf, AppError> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.file_name().and_then(|e| e.to_str()) == Some("model.plan") {
            return Ok(path);
        }
    }
    Err(AppError::Conversion(format!(
        "no model.plan file found in {}",
        dir.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_tracker_only_accepts_increases() {
        let mut tracker = ProgressTracker::new(5);

        assert_eq!(tracker.next_update(Some(5)), None);
        assert_eq!(tracker.next_update(Some(20)), Some(20));
        assert_eq!(tracker.next_update(Some(20)), None);
        assert_eq!(tracker.next_update(Some(10)), None);
        assert_eq!(tracker.next_update(Some(70)), Some(70));
        assert_eq!(tracker.next_update(None), None);
    }

    #[test]
    fn parse_progress_uses_known_trtexec_milestones() {
        assert_eq!(parse_progress("[I] Building engine"), Some(20));
        assert_eq!(parse_progress("[I] Finished engine building"), Some(70));
        assert_eq!(parse_progress("[I] Starting conversion"), None);
    }
}
