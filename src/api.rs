//! Dioxus `#[server]` functions — the HTTP interface between frontend and backend.
//!
//! This module is compiled for **both** native (server) and WASM (client) targets.
//! The `#[server]` macro generates the full implementation on native and a thin HTTP
//! client stub on WASM. Server-only imports and state are guarded by
//! `#[cfg(not(target_arch = "wasm32"))]`.

use dioxus::prelude::*;

// Shared types used in the function signatures (compiled on all targets).
use crate::models::config::{GpuInfo, TensorRtImage};
use crate::models::job::{ConversionJob, JobId, SubmitJobRequest};
use crate::models::template::ConfigTemplate;

// ---------------------------------------------------------------------------
// Server-only imports, state, and helpers
// ---------------------------------------------------------------------------

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
use {
    crate::errors::AppError,
    crate::models::config::{AppConfig, GpuId, load_dotenv},
    crate::models::job::{ModelFormat, TrtOptions},
    crate::server::db::{self, DbPool},
    crate::server::docker::DockerService,
    crate::server::storage::StorageService,
    std::collections::HashSet,
    tokio::sync::OnceCell,
};

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
static DB_POOL: OnceCell<DbPool> = OnceCell::const_new();
#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
static DOCKER_SERVICE: OnceCell<DockerService> = OnceCell::const_new();
#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
static APP_CONFIG: std::sync::OnceLock<AppConfig> = std::sync::OnceLock::new();

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn app_config() -> &'static AppConfig {
    APP_CONFIG.get_or_init(AppConfig::from_env)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
async fn db_pool() -> Result<&'static DbPool, AppError> {
    DB_POOL
        .get_or_try_init(|| async {
            load_dotenv();
            let url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/converter.db".into());
            db::init_db(&url).await
        })
        .await
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
async fn docker_service() -> Result<&'static DockerService, AppError> {
    DOCKER_SERVICE.get_or_try_init(DockerService::new).await
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn storage_service() -> StorageService {
    StorageService::new(app_config())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn to_server_err(e: AppError) -> ServerFnError {
    ServerFnError::new(e.to_string())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn model_ext(format: &ModelFormat) -> &'static str {
    match format {
        ModelFormat::Onnx => "onnx",
        ModelFormat::TensorFlowSavedModel => "savedmodel",
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn build_new_job(
    model_name: String,
    model_format: ModelFormat,
    image_tag: String,
    gpu_id: GpuId,
    template_name: String,
    trt_options: TrtOptions,
) -> ConversionJob {
    let now = chrono::Utc::now();
    ConversionJob {
        id: JobId::new(),
        model_name,
        model_format,
        image_tag,
        gpu_id,
        template_name,
        trt_options,
        status: crate::models::job::JobStatus::Pending,
        progress_percent: 0,
        output_path: None,
        error_message: None,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn parse_job_id(s: &str) -> Result<JobId, AppError> {
    s.parse::<uuid::Uuid>()
        .map(JobId)
        .map_err(|e| AppError::Validation(format!("invalid job id '{s}': {e}")))
}

// ---------------------------------------------------------------------------
// Server functions (compiled for both targets via #[server] macro)
// ---------------------------------------------------------------------------

/// Returns the TensorRT Docker images present in the local image cache,
/// falling back to a curated list of known images when Docker is unavailable.
#[server]
#[tracing::instrument(skip_all)]
pub async fn get_available_images() -> Result<Vec<TensorRtImage>, ServerFnError> {
    let local_images = match docker_service().await {
        Ok(docker) => docker.list_tensorrt_images().await.unwrap_or_default(),
        Err(e) => {
            tracing::warn!(error = ?e, "Docker unavailable; returning configured image list");
            vec![]
        }
    };

    let configured = configured_tensorrt_images().await.map_err(to_server_err)?;
    Ok(merge_image_lists(
        local_images,
        configured,
        curated_tensorrt_images(),
    ))
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
#[derive(Debug, serde::Deserialize)]
struct TensorRtImagesConfig {
    images: Vec<TensorRtImage>,
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
async fn configured_tensorrt_images() -> Result<Vec<TensorRtImage>, AppError> {
    load_dotenv();
    let path = std::env::var("TENSORRT_IMAGES_CONFIG")
        .unwrap_or_else(|_| "config/images.toml".to_string());
    let path = std::path::PathBuf::from(path);

    match tokio::fs::read_to_string(&path).await {
        Ok(contents) => toml::from_str::<TensorRtImagesConfig>(&contents)
            .map(|config| config.images)
            .map_err(AppError::Config),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(AppError::Io(e)),
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn merge_image_lists(
    local: Vec<TensorRtImage>,
    configured: Vec<TensorRtImage>,
    curated: Vec<TensorRtImage>,
) -> Vec<TensorRtImage> {
    let mut seen = HashSet::new();
    local
        .into_iter()
        .chain(configured)
        .chain(curated)
        .filter(|image| seen.insert(image.tag.clone()))
        .collect()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn curated_tensorrt_images() -> Vec<TensorRtImage> {
    vec![
        TensorRtImage {
            name: "TensorRT 10.3 — CUDA 12.6 (latest)".into(),
            tag: "nvcr.io/nvidia/tensorrt:24.08-py3".into(),
            cuda_version: "12.6".into(),
            tensorrt_version: "10.3".into(),
        },
        TensorRtImage {
            name: "TensorRT 10.0 — CUDA 12.4".into(),
            tag: "nvcr.io/nvidia/tensorrt:24.04-py3".into(),
            cuda_version: "12.4".into(),
            tensorrt_version: "10.0".into(),
        },
        TensorRtImage {
            name: "TensorRT 9.3 — CUDA 12.2".into(),
            tag: "nvcr.io/nvidia/tensorrt:23.12-py3".into(),
            cuda_version: "12.2".into(),
            tensorrt_version: "9.3".into(),
        },
        TensorRtImage {
            name: "TensorRT 8.6 — CUDA 12.0".into(),
            tag: "nvcr.io/nvidia/tensorrt:23.04-py3".into(),
            cuda_version: "12.0".into(),
            tensorrt_version: "8.6".into(),
        },
    ]
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
#[tracing::instrument(skip_all, fields(model_name = %req.model_name, image_tag = %req.image_tag))]
pub async fn submit_job(
    model_data: Vec<u8>,
    req: SubmitJobRequest,
) -> Result<JobId, ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    let docker = docker_service().await.map_err(to_server_err)?;
    let storage = storage_service();

    // Resource check: free GPU memory must be > 1.4 * workspace_mb
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        use crate::server::gpu::GpuService;
        let gpu_svc = GpuService::new();
        let gpus = gpu_svc.detect_gpus().await;
        let selected_gpu = gpus.iter().find(|g| g.id == GpuId(req.gpu_id));

        match selected_gpu {
            Some(gpu) => {
                let required_mb = (req.trt_options.workspace_mb as f64 * 1.4) as u64;
                if gpu.memory_free_mb < required_mb {
                    return Err(to_server_err(AppError::Validation(format!(
                        "Insufficient GPU memory: {} MB free, but {} MB required (1.4 * workspace)",
                        gpu.memory_free_mb, required_mb
                    ))));
                }
            }
            None if gpus.is_empty() => {
                tracing::warn!(
                    gpu_id = req.gpu_id,
                    "GPU detection unavailable; accepting manually selected GPU"
                );
            }
            None => {
                return Err(to_server_err(AppError::Validation(format!(
                    "GPU device {} not found",
                    req.gpu_id
                ))));
            }
        }
    }

    let filename = format!("{}.{}", req.model_name, model_ext(&req.model_format));
    let (model_path, _) = storage
        .save_upload(&filename, &model_data)
        .await
        .map_err(to_server_err)?;

    let job = build_new_job(
        req.model_name,
        req.model_format,
        req.image_tag,
        GpuId(req.gpu_id),
        req.template_name,
        req.trt_options,
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
    let server_path = req.server_output_path.map(std::path::PathBuf::from);
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
