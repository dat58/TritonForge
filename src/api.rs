//! Dioxus `#[server]` functions — the HTTP interface between frontend and backend.
//!
//! This module is compiled for **both** native (server) and WASM (client) targets.
//! The `#[server]` macro generates the full implementation on native and a thin HTTP
//! client stub on WASM. Server-only imports and state are guarded by
//! `#[cfg(not(target_arch = "wasm32"))]`.

use dioxus::prelude::*;

// Shared types used in the function signatures (compiled on all targets).
use crate::models::config::{GpuInfo, TensorRtImage};
use crate::models::group::{GroupId, ModelGroup, ModelGroupMember};
use crate::models::job::{ConversionJob, JobId, SubmitJobRequest};

// ---------------------------------------------------------------------------
// Server-only imports, state, and helpers
// ---------------------------------------------------------------------------

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
use {
    crate::errors::AppError,
    crate::models::config::{AppConfig, GpuId, load_dotenv},
    crate::models::group::random_mythology_name,
    crate::models::job::{JobStatus, ModelFormat, TrtOptions},
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
fn build_new_job(
    model_name: String,
    model_version: u32,
    image_tag: String,
    gpu_id: GpuId,
    trt_options: TrtOptions,
) -> ConversionJob {
    let now = chrono::Utc::now();
    ConversionJob {
        id: JobId::new(),
        model_name,
        model_version,
        model_format: ModelFormat::Onnx,
        image_tag,
        gpu_id,
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

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn validate_submit_request(req: &SubmitJobRequest) -> Result<(), AppError> {
    validate_model_name(&req.model_name)?;
    if req.model_version == 0 {
        return Err(AppError::Validation(
            "model version must be at least 1".into(),
        ));
    }
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn validate_model_name(name: &str) -> Result<(), AppError> {
    let valid = !name.trim().is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'));

    if valid {
        Ok(())
    } else {
        Err(AppError::Validation(
            "model name must contain only letters, numbers, '.', '_', or '-'".into(),
        ))
    }
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
    vec![]
}

/// Returns all NVIDIA GPUs detected by nvidia-smi.
#[server]
#[tracing::instrument(skip_all)]
pub async fn get_available_gpus() -> Result<Vec<GpuInfo>, ServerFnError> {
    use crate::server::gpu::GpuService;
    let svc = GpuService::new();
    Ok(svc.detect_gpus().await)
}

/// Saves the uploaded model to disk and schedules a TensorRT conversion job.
///
/// Returns the newly created `JobId` for progress polling.
#[server(input = Cbor, output = Cbor)]
#[tracing::instrument(skip_all, fields(model_name = %req.model_name, image_tag = %req.image_tag))]
pub async fn submit_job(
    model_data: Vec<u8>,
    req: SubmitJobRequest,
) -> Result<JobId, ServerFnError> {
    validate_submit_request(&req).map_err(to_server_err)?;

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

    let filename = format!("{}.onnx", req.model_name);
    let (model_path, _) = storage
        .save_upload(&filename, &model_data)
        .await
        .map_err(to_server_err)?;

    let job = build_new_job(
        req.model_name,
        req.model_version,
        req.image_tag,
        GpuId(req.gpu_id),
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
    let docker_clone = docker.clone();

    tokio::spawn(async move {
        let storage_inner = storage_service();
        let config = app_config();
        let conv = ConversionService::new(docker_clone, storage_inner.clone(), pool_arc, config);

        match conv.run_conversion(job, model_path).await {
            Ok(_) => tracing::info!("background conversion finished"),
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

/// Returns a zip archive of the completed Triton model folder.
#[server(output = Cbor)]
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
    let model_dir = storage
        .get_model_dir(&jid, &job.model_name)
        .await
        .map_err(to_server_err)?;

    storage
        .zip_model_dir(&model_dir, &job.model_name)
        .await
        .map_err(to_server_err)
}

/// Returns persisted container logs for a conversion job.
#[server]
#[tracing::instrument(skip_all, fields(job_id, limit))]
pub async fn get_job_logs(job_id: String, limit: u32) -> Result<String, ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    let jid = parse_job_id(&job_id).map_err(to_server_err)?;
    let capped_limit = limit.clamp(1, 1_000);
    let rows = db::list_job_logs(pool, &jid, capped_limit)
        .await
        .map_err(to_server_err)?;
    let mut logs = String::new();

    for row in rows {
        logs.push_str(&row.message);
        logs.push('\n');
    }

    Ok(logs)
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

/// Deletes a completed or failed job row, its logs, and its output directory.
#[server]
#[tracing::instrument(skip_all, fields(job_id))]
pub async fn delete_job(job_id: String) -> Result<(), ServerFnError> {
    let pool = db_pool().await.map_err(to_server_err)?;
    let jid = parse_job_id(&job_id).map_err(to_server_err)?;
    let job = db::get_job(pool, &jid).await.map_err(to_server_err)?;

    if !matches!(job.status, JobStatus::Completed | JobStatus::Failed) {
        return Err(to_server_err(AppError::Validation(
            "only completed or failed jobs can be deleted".into(),
        )));
    }

    let storage = storage_service();
    storage
        .delete_job_output_root(&jid)
        .await
        .map_err(to_server_err)?;
    db::delete_job_logs(pool, &jid)
        .await
        .map_err(to_server_err)?;
    db::delete_job(pool, &jid).await.map_err(to_server_err)
}

// ── Model group server functions ──────────────────────────────────────────────

/// Creates a new model group, defaulting to a random mythology name.
#[server]
pub async fn create_model_group(name: Option<String>) -> Result<ModelGroup, ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        let pool = db_pool().await.map_err(to_server_err)?;
        let storage = storage_service();
        let group_name = name
            .map(|n| n.trim().to_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(random_mythology_name);
        let dir_path = storage.groups_dir().join(&group_name);
        let now = chrono::Utc::now();
        let group = ModelGroup {
            id: GroupId::new(),
            name: group_name,
            dir_path,
            members: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        db::insert_group(pool, &group)
            .await
            .map_err(to_server_err)?;
        Ok(group)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}

/// Returns all model groups ordered by creation time (newest first).
#[server]
pub async fn list_model_groups() -> Result<Vec<ModelGroup>, ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        let pool = db_pool().await.map_err(to_server_err)?;
        db::list_groups(pool).await.map_err(to_server_err)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}

/// Renames an existing group.
#[server]
pub async fn rename_model_group(group_id: GroupId, name: String) -> Result<(), ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        let trimmed = name.trim().to_owned();
        if trimmed.is_empty() {
            return Err(ServerFnError::new("group name cannot be empty"));
        }
        let pool = db_pool().await.map_err(to_server_err)?;
        db::update_group_name(pool, &group_id, &trimmed)
            .await
            .map_err(to_server_err)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}

/// Copies selected models into a group and records the membership.
#[server]
pub async fn add_models_to_group(
    group_id: GroupId,
    members: Vec<ModelGroupMember>,
) -> Result<ModelGroup, ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        let pool = db_pool().await.map_err(to_server_err)?;
        let storage = storage_service();
        let group = db::get_group(pool, &group_id)
            .await
            .map_err(to_server_err)?;

        for member in &members {
            if group
                .members
                .iter()
                .any(|m| m.model_name == member.model_name)
            {
                return Err(ServerFnError::new(format!(
                    "model '{}' already exists in group '{}'",
                    member.model_name, group.name
                )));
            }
        }

        for member in &members {
            storage
                .copy_model_to_group(&member.job_id, &member.model_name, &group.name)
                .await
                .map_err(to_server_err)?;
            db::add_group_member(pool, &group_id, member)
                .await
                .map_err(to_server_err)?;
        }

        db::get_group(pool, &group_id).await.map_err(to_server_err)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}

/// Removes a model from a group (DB record only — no file deletion).
#[server]
pub async fn remove_model_from_group(
    group_id: GroupId,
    model_name: String,
) -> Result<(), ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        validate_model_name(&model_name).map_err(to_server_err)?;
        let pool = db_pool().await.map_err(to_server_err)?;
        let storage = storage_service();
        let group = db::get_group(pool, &group_id)
            .await
            .map_err(to_server_err)?;
        storage
            .delete_group_model_dir(&group.name, &model_name)
            .await
            .map_err(to_server_err)?;
        db::remove_group_member(pool, &group_id, &model_name)
            .await
            .map_err(to_server_err)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}

/// Deletes the group's storage directory and the DB record (source files are preserved).
#[server]
pub async fn release_model_group(group_id: GroupId) -> Result<(), ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        let pool = db_pool().await.map_err(to_server_err)?;
        let storage = storage_service();
        let group = db::get_group(pool, &group_id)
            .await
            .map_err(to_server_err)?;
        storage
            .delete_group_dir(&group.name)
            .await
            .map_err(to_server_err)?;
        db::delete_group(pool, &group_id)
            .await
            .map_err(to_server_err)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}

/// Deletes the group directory, each member's original source files, and the DB record.
#[server]
pub async fn delete_model_group(group_id: GroupId) -> Result<(), ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        let pool = db_pool().await.map_err(to_server_err)?;
        let storage = storage_service();
        let group = db::get_group(pool, &group_id)
            .await
            .map_err(to_server_err)?;
        storage
            .delete_group_dir(&group.name)
            .await
            .map_err(to_server_err)?;
        for member in &group.members {
            storage
                .delete_job_output_dir(&member.job_id, &member.model_name)
                .await
                .map_err(to_server_err)?;
        }
        db::delete_group(pool, &group_id)
            .await
            .map_err(to_server_err)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}

/// Returns all conversion jobs with Completed status, newest first.
#[server]
pub async fn list_completed_jobs() -> Result<Vec<ConversionJob>, ServerFnError> {
    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    {
        let pool = db_pool().await.map_err(to_server_err)?;
        db::list_completed_jobs(pool).await.map_err(to_server_err)
    }
    #[cfg(not(all(not(target_arch = "wasm32"), feature = "server")))]
    unreachable!()
}
