//! Dioxus `#[server]` functions — the HTTP interface between frontend and backend.
//!
//! This module is compiled for **both** native (server) and WASM (client) targets.
//! The `#[server]` macro generates the full implementation on native and a thin HTTP
//! client stub on WASM. Server-only imports and state are guarded by
//! `#[cfg(not(target_arch = "wasm32"))]`.

use dioxus::prelude::*;

// Shared types used in the function signatures (compiled on all targets).
use crate::models::config::{GpuInfo, TensorRtImage};
use crate::models::job::{ConversionJob, JobId, ModelFormat};
use crate::models::template::ConfigTemplate;

// ---------------------------------------------------------------------------
// Server-only imports, state, and helpers
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
use {
    crate::errors::AppError,
    crate::models::config::{AppConfig, GpuId},
    crate::server::db::{self, DbPool},
    crate::server::docker::DockerService,
    crate::server::storage::StorageService,
    tokio::sync::OnceCell,
};

#[cfg(not(target_arch = "wasm32"))]
static DB_POOL: OnceCell<DbPool> = OnceCell::const_new();
#[cfg(not(target_arch = "wasm32"))]
static DOCKER_SERVICE: OnceCell<DockerService> = OnceCell::const_new();
#[cfg(not(target_arch = "wasm32"))]
static APP_CONFIG: std::sync::OnceLock<AppConfig> = std::sync::OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn app_config() -> &'static AppConfig {
    APP_CONFIG.get_or_init(AppConfig::from_env)
}

#[cfg(not(target_arch = "wasm32"))]
async fn db_pool() -> Result<&'static DbPool, AppError> {
    DB_POOL
        .get_or_try_init(|| async {
            let url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/converter.db".into());
            db::init_db(&url).await
        })
        .await
}

#[cfg(not(target_arch = "wasm32"))]
async fn docker_service() -> Result<&'static DockerService, AppError> {
    DOCKER_SERVICE.get_or_try_init(DockerService::new).await
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_service() -> StorageService {
    StorageService::new(app_config())
}

#[cfg(not(target_arch = "wasm32"))]
fn to_server_err(e: AppError) -> ServerFnError {
    ServerFnError::new(e.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn model_ext(format: &ModelFormat) -> &'static str {
    match format {
        ModelFormat::Onnx => "onnx",
        ModelFormat::TensorFlowSavedModel => "savedmodel",
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_new_job(
    model_name: String,
    model_format: ModelFormat,
    image_tag: String,
    gpu_id: GpuId,
    template_name: String,
) -> ConversionJob {
    let now = chrono::Utc::now();
    ConversionJob {
        id: JobId::new(),
        model_name,
        model_format,
        image_tag,
        gpu_id,
        template_name,
        status: crate::models::job::JobStatus::Pending,
        progress_percent: 0,
        output_path: None,
        error_message: None,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_job_id(s: &str) -> Result<JobId, AppError> {
    s.parse::<uuid::Uuid>()
        .map(JobId)
        .map_err(|e| AppError::Validation(format!("invalid job id '{s}': {e}")))
}

// ---------------------------------------------------------------------------
// Server functions (compiled for both targets via #[server] macro)
// ---------------------------------------------------------------------------

/// Returns the TensorRT Docker images present in the local image cache.
#[server]
#[tracing::instrument(skip_all)]
pub async fn get_available_images() -> Result<Vec<TensorRtImage>, ServerFnError> {
    let docker = docker_service().await.map_err(to_server_err)?;
    docker.list_tensorrt_images().await.map_err(to_server_err)
}

/// Returns all NVIDIA GPUs detected by nvidia-smi.
#[server]
#[tracing::instrument(skip_all)]
pub async fn get_available_gpus() -> Result<Vec<GpuInfo>, ServerFnError> {
    use crate::server::gpu::GpuService;
    let svc = GpuService::new();
    Ok(svc.detect_gpus().await)
}

/// Returns all `.pbtxt` config templates from the templates directory.
#[server]
#[tracing::instrument(skip_all)]
pub async fn get_available_templates() -> Result<Vec<ConfigTemplate>, ServerFnError> {
    let dir = std::path::PathBuf::from(
        std::env::var("TEMPLATES_DIR").unwrap_or_else(|_| "./templates".into()),
    );
    crate::models::template::load_templates(&dir)
        .await
        .map_err(to_server_err)
}

/// Saves the uploaded model to disk and schedules a TensorRT conversion job.
///
/// Returns the newly created `JobId` for progress polling.
#[server]
#[tracing::instrument(skip_all, fields(model_name, image_tag))]
pub async fn submit_job(
    model_data: Vec<u8>,
    model_name: String,
    model_format: ModelFormat,
    image_tag: String,
    gpu_id: u32,
    template_name: String,
    server_output_path: Option<String>,
) -> Result<JobId, ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    let docker = docker_service().await.map_err(to_server_err)?;
    let storage = storage_service();

    let filename = format!("{model_name}.{}", model_ext(&model_format));
    let (model_path, _) = storage
        .save_upload(&filename, &model_data)
        .await
        .map_err(to_server_err)?;

    let job = build_new_job(
        model_name,
        model_format,
        image_tag,
        GpuId(gpu_id),
        template_name,
    );
    let job_id = job.id.clone();

    db::insert_job(pool, &job).await.map_err(to_server_err)?;

    if !docker
        .is_image_available(&job.image_tag)
        .await
        .map_err(to_server_err)?
    {
        docker
            .pull_image(&job.image_tag)
            .await
            .map_err(to_server_err)?;
    }

    use {crate::server::conversion::ConversionService, std::sync::Arc};
    let pool_arc = Arc::new(pool.clone());
    let server_path = server_output_path.map(std::path::PathBuf::from);
    let docker_clone = docker.clone();

    tokio::spawn(async move {
        let storage_inner = storage_service();
        let config = app_config();
        let conv = ConversionService::new(docker_clone, storage_inner.clone(), pool_arc, config);

        match conv.run_conversion(job, model_path).await {
            Ok(engine_path) => {
                if let Some(target) = server_path {
                    let _ = storage_inner
                        .save_to_server_path(&engine_path, &target)
                        .await;
                }
                tracing::info!("background conversion finished");
            }
            Err(e) => {
                tracing::error!(error = ?e, "background conversion failed");
            }
        }
    });

    Ok(job_id)
}

/// Returns the current state of a single conversion job.
#[server]
#[tracing::instrument(skip_all, fields(job_id))]
pub async fn get_job_status(job_id: String) -> Result<ConversionJob, ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    let jid = parse_job_id(&job_id).map_err(to_server_err)?;
    db::get_job(pool, &jid).await.map_err(to_server_err)
}

/// Returns a paginated list of all conversion jobs, newest first.
#[server]
#[tracing::instrument(skip_all, fields(limit, offset))]
pub async fn list_all_jobs(limit: u32, offset: u32) -> Result<Vec<ConversionJob>, ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    db::list_jobs(pool, limit, offset)
        .await
        .map_err(to_server_err)
}

/// Returns the raw engine bytes for a completed job.
#[server]
#[tracing::instrument(skip_all, fields(job_id))]
pub async fn download_model(job_id: String) -> Result<Vec<u8>, ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    let jid = parse_job_id(&job_id).map_err(to_server_err)?;

    let job = db::get_job(pool, &jid).await.map_err(to_server_err)?;
    if job.output_path.is_none() {
        return Err(to_server_err(AppError::Validation(
            "job not yet completed".into(),
        )));
    }

    let storage = storage_service();
    let path = storage
        .get_download_path(&jid)
        .await
        .map_err(to_server_err)?;

    tokio::fs::read(&path)
        .await
        .map_err(|e| to_server_err(AppError::Io(e)))
}

/// Stops a running conversion job by stopping its Docker container.
#[server]
#[tracing::instrument(skip_all, fields(job_id))]
pub async fn cancel_job(job_id: String) -> Result<(), ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    let jid = parse_job_id(&job_id).map_err(to_server_err)?;
    let docker = docker_service().await.map_err(to_server_err)?;

    let container_name = format!("tritonforge-{jid}");

    match docker
        .client()
        .stop_container(
            &container_name,
            Some(
                bollard::query_parameters::StopContainerOptionsBuilder::default()
                    .t(5)
                    .build(),
            ),
        )
        .await
    {
        Ok(())
        | Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => {}
        Err(e) => return Err(to_server_err(AppError::Docker(e))),
    }

    db::update_job_failed(pool, &jid, "cancelled by user")
        .await
        .map_err(to_server_err)
}
