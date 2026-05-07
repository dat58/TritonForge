//! SQLite persistence layer: connection pool initialisation and CRUD helpers.

use crate::errors::AppError;
use crate::models::config::GpuId;
use crate::models::group::{GroupId, ModelGroup, ModelGroupMember};
use crate::models::job::{
    ConversionJob, ConversionJobLog, JobId, JobStatus, ModelFormat, TrtOptions, WarmupInput,
};
use crate::models::serving::{ServingContainer, ServingStatus};
use chrono::DateTime;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{FromRow, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tracing::instrument;

/// Shared connection pool alias.
pub type DbPool = SqlitePool;

/// Connects to the SQLite database and runs any pending migrations.
///
/// `database_url` must be a valid SQLite URL such as `sqlite://data/converter.db`
/// or the special `sqlite::memory:` for in-memory testing.
/// File-backed databases use rollback journaling so committed rows are stored
/// in the main database file across service/container restarts.
#[instrument(skip_all, fields(database_url))]
pub async fn init_db(database_url: &str) -> Result<DbPool, AppError> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(10));
    create_database_parent_dir(options.get_filename()).await?;

    let options = apply_persistent_file_options(options);

    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(|e| AppError::Conversion(format!("migration failed: {e}")))?;
    tracing::info!(database_url, "database pool initialised");
    Ok(pool)
}

fn apply_persistent_file_options(options: SqliteConnectOptions) -> SqliteConnectOptions {
    if options.get_filename() == std::path::Path::new(":memory:") {
        return options;
    }

    options.journal_mode(SqliteJournalMode::Delete)
}

async fn create_database_parent_dir(path: &std::path::Path) -> Result<(), AppError> {
    if path == std::path::Path::new(":memory:") {
        return Ok(());
    }

    let Some(parent) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) else {
        return Ok(());
    };

    tokio::fs::create_dir_all(parent).await?;
    Ok(())
}

/// Raw row returned by SQLite queries — converted to [`ConversionJob`] before leaving this module.
#[derive(Debug, FromRow)]
struct ConversionJobRow {
    id: String,
    model_name: String,
    model_version: i64,
    model_format: String,
    image_tag: String,
    gpu_id: i64,
    trt_options: String,
    warmup_inputs: String,
    status: String,
    progress_percent: i64,
    output_path: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

fn row_to_job(row: ConversionJobRow) -> Result<ConversionJob, AppError> {
    let id_uuid = row
        .id
        .parse()
        .map_err(|e| AppError::Validation(format!("invalid job id: {e}")))?;

    let gpu_raw = u32::try_from(row.gpu_id)
        .map_err(|_| AppError::Validation("gpu_id out of u32 range".into()))?;
    let model_version = u32::try_from(row.model_version)
        .map_err(|_| AppError::Validation("model_version out of u32 range".into()))?;

    let created_at = DateTime::parse_from_rfc3339(&row.created_at)
        .map_err(|e| AppError::Validation(format!("invalid created_at: {e}")))?
        .to_utc();

    let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
        .map_err(|e| AppError::Validation(format!("invalid updated_at: {e}")))?
        .to_utc();

    let trt_options: TrtOptions = serde_json::from_str(&row.trt_options)
        .map_err(|e| AppError::Conversion(format!("failed to parse trt_options: {e}")))?;

    let warmup_inputs: Vec<WarmupInput> = serde_json::from_str(&row.warmup_inputs)
        .map_err(|e| AppError::Conversion(format!("failed to parse warmup_inputs: {e}")))?;

    Ok(ConversionJob {
        id: JobId(id_uuid),
        model_name: row.model_name,
        model_version,
        model_format: ModelFormat::from_str(&row.model_format)?,
        image_tag: row.image_tag,
        gpu_id: GpuId(gpu_raw),
        trt_options,
        status: JobStatus::from_str(&row.status)?,
        progress_percent: checked_progress(row.progress_percent)?,
        output_path: row.output_path.map(PathBuf::from),
        error_message: row.error_message,
        warmup_inputs,
        created_at,
        updated_at,
    })
}

fn checked_progress(progress: i64) -> Result<u8, AppError> {
    u8::try_from(progress)
        .map(|value| value.min(100))
        .map_err(|_| AppError::Validation("progress_percent out of u8 range".into()))
}

/// New conversion log line ready for insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewJobLog {
    /// Container stream name, usually `stdout` or `stderr`.
    pub stream: String,
    /// Log line text.
    pub message: String,
}

impl NewJobLog {
    /// Builds a new persisted log row payload.
    pub fn new(stream: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stream: stream.into(),
            message: message.into(),
        }
    }
}

/// Inserts a new job record into the database.
#[instrument(skip(pool), fields(job_id = %job.id))]
pub async fn insert_job(pool: &DbPool, job: &ConversionJob) -> Result<(), AppError> {
    let trt_options_json = serde_json::to_string(&job.trt_options)
        .map_err(|e| AppError::Conversion(format!("failed to serialize trt_options: {e}")))?;

    let warmup_json = serde_json::to_string(&job.warmup_inputs)
        .map_err(|e| AppError::Conversion(format!("failed to serialize warmup_inputs: {e}")))?;

    sqlx::query(
        "INSERT INTO conversion_jobs \
         (id, model_name, model_version, model_format, image_tag, gpu_id, template_name, trt_options, \
          warmup_inputs, status, progress_percent, output_path, error_message, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job.id.to_string())
    .bind(&job.model_name)
    .bind(i64::from(job.model_version))
    .bind(job.model_format.to_string())
    .bind(&job.image_tag)
    .bind(i64::from(job.gpu_id.0))
    .bind("config")
    .bind(trt_options_json)
    .bind(warmup_json)
    .bind(job.status.to_string())
    .bind(i64::from(job.progress_percent))
    .bind(
        job.output_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
    )
    .bind(&job.error_message)
    .bind(job.created_at.to_rfc3339())
    .bind(job.updated_at.to_rfc3339())
    .execute(pool)
    .await?;

    tracing::info!("job inserted");
    Ok(())
}

/// Updates a job's status and progress percentage.
#[instrument(skip(pool), fields(job_id = %job_id, %status, progress))]
pub async fn update_job_status(
    pool: &DbPool,
    job_id: &JobId,
    status: JobStatus,
    progress: u8,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE conversion_jobs \
         SET status = ?, progress_percent = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(status.to_string())
    .bind(i64::from(progress))
    .bind(&now)
    .bind(job_id.to_string())
    .execute(pool)
    .await?;

    tracing::debug!(progress, "job status updated");
    Ok(())
}

/// Marks a job as successfully completed and records the output engine path.
#[instrument(skip(pool), fields(job_id = %job_id))]
pub async fn update_job_completed(
    pool: &DbPool,
    job_id: &JobId,
    output_path: &std::path::Path,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE conversion_jobs \
         SET status = 'completed', progress_percent = 100, output_path = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(output_path.to_string_lossy().as_ref())
    .bind(&now)
    .bind(job_id.to_string())
    .execute(pool)
    .await?;

    tracing::info!("job completed");
    Ok(())
}

/// Marks a job as failed and stores the diagnostic error message.
#[instrument(skip(pool), fields(job_id = %job_id))]
pub async fn update_job_failed(
    pool: &DbPool,
    job_id: &JobId,
    error_message: &str,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE conversion_jobs \
         SET status = 'failed', error_message = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(error_message)
    .bind(&now)
    .bind(job_id.to_string())
    .execute(pool)
    .await?;

    tracing::error!(error_message, "job failed");
    Ok(())
}

/// Fetches a single job by its ID.
#[instrument(skip(pool), fields(job_id = %job_id))]
pub async fn get_job(pool: &DbPool, job_id: &JobId) -> Result<ConversionJob, AppError> {
    let row = sqlx::query_as::<_, ConversionJobRow>(
        "SELECT id, model_name, model_version, model_format, image_tag, gpu_id, trt_options, \
         warmup_inputs, status, progress_percent, output_path, error_message, created_at, updated_at \
         FROM conversion_jobs WHERE id = ?",
    )
    .bind(job_id.to_string())
    .fetch_one(pool)
    .await?;

    row_to_job(row)
}

/// Returns a page of jobs ordered by creation time (newest first).
#[instrument(skip(pool), fields(limit, offset))]
pub async fn list_jobs(
    pool: &DbPool,
    limit: u32,
    offset: u32,
) -> Result<Vec<ConversionJob>, AppError> {
    let rows = sqlx::query_as::<_, ConversionJobRow>(
        "SELECT id, model_name, model_version, model_format, image_tag, gpu_id, trt_options, \
         warmup_inputs, status, progress_percent, output_path, error_message, created_at, updated_at \
         FROM conversion_jobs \
         ORDER BY created_at DESC \
         LIMIT ? OFFSET ?",
    )
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(row_to_job).collect()
}

/// Returns all completed jobs ordered by creation time (newest first), capped at 200.
#[instrument(skip(pool))]
pub async fn list_completed_jobs(pool: &DbPool) -> Result<Vec<ConversionJob>, AppError> {
    let rows = sqlx::query_as::<_, ConversionJobRow>(
        "SELECT id, model_name, model_version, model_format, image_tag, gpu_id, trt_options, \
         warmup_inputs, status, progress_percent, output_path, error_message, created_at, updated_at \
         FROM conversion_jobs \
         WHERE status = 'completed' \
         ORDER BY created_at DESC \
         LIMIT 200",
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(row_to_job).collect()
}

/// Deletes all persisted logs for a job.
#[instrument(skip(pool), fields(job_id = %job_id))]
pub async fn delete_job_logs(pool: &DbPool, job_id: &JobId) -> Result<(), AppError> {
    sqlx::query("DELETE FROM conversion_job_logs WHERE job_id = ?")
        .bind(job_id.to_string())
        .execute(pool)
        .await?;

    tracing::debug!("job logs deleted");
    Ok(())
}

/// Deletes a conversion job row.
#[instrument(skip(pool), fields(job_id = %job_id))]
pub async fn delete_job(pool: &DbPool, job_id: &JobId) -> Result<(), AppError> {
    sqlx::query("DELETE FROM conversion_jobs WHERE id = ?")
        .bind(job_id.to_string())
        .execute(pool)
        .await?;

    tracing::info!("job deleted");
    Ok(())
}

#[derive(Debug, FromRow)]
struct ConversionJobLogRow {
    id: i64,
    job_id: String,
    stream: String,
    message: String,
    created_at: String,
}

fn row_to_log(row: ConversionJobLogRow) -> Result<ConversionJobLog, AppError> {
    let job_uuid = row
        .job_id
        .parse()
        .map_err(|e| AppError::Validation(format!("invalid log job id: {e}")))?;
    let created_at = DateTime::parse_from_rfc3339(&row.created_at)
        .map_err(|e| AppError::Validation(format!("invalid log created_at: {e}")))?
        .to_utc();

    Ok(ConversionJobLog {
        id: row.id,
        job_id: JobId(job_uuid),
        stream: row.stream,
        message: row.message,
        created_at,
    })
}

/// Appends a batch of container log lines for a conversion job.
#[instrument(skip(pool, logs), fields(job_id = %job_id, count = logs.len()))]
pub async fn append_job_logs_batch(
    pool: &DbPool,
    job_id: &JobId,
    logs: &[NewJobLog],
) -> Result<(), AppError> {
    if logs.is_empty() {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;

    for log in logs {
        sqlx::query(
            "INSERT INTO conversion_job_logs (job_id, stream, message, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(job_id.to_string())
        .bind(&log.stream)
        .bind(&log.message)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    tracing::debug!(count = logs.len(), "job logs appended");
    Ok(())
}

/// Lists the most recent persisted logs for a job in chronological order.
#[instrument(skip(pool), fields(job_id = %job_id, limit))]
pub async fn list_job_logs(
    pool: &DbPool,
    job_id: &JobId,
    limit: u32,
) -> Result<Vec<ConversionJobLog>, AppError> {
    let rows = sqlx::query_as::<_, ConversionJobLogRow>(
        "SELECT id, job_id, stream, message, created_at \
         FROM conversion_job_logs \
         WHERE job_id = ? \
         ORDER BY id DESC \
         LIMIT ?",
    )
    .bind(job_id.to_string())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    rows.into_iter().rev().map(row_to_log).collect()
}

// ── Model group CRUD ──────────────────────────────────────────────────────────

#[derive(Debug, FromRow)]
struct ModelGroupRow {
    id: String,
    name: String,
    dir_path: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct ModelGroupMemberRow {
    job_id: String,
    model_name: String,
}

fn row_to_group(
    row: ModelGroupRow,
    members: Vec<ModelGroupMember>,
) -> Result<ModelGroup, AppError> {
    let id = row
        .id
        .parse()
        .map_err(|e| AppError::Validation(format!("invalid group id: {e}")))?;
    let created_at = DateTime::parse_from_rfc3339(&row.created_at)
        .map_err(|e| AppError::Validation(format!("invalid group created_at: {e}")))?
        .to_utc();
    let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
        .map_err(|e| AppError::Validation(format!("invalid group updated_at: {e}")))?
        .to_utc();
    Ok(ModelGroup {
        id,
        name: row.name,
        dir_path: PathBuf::from(row.dir_path),
        members,
        created_at,
        updated_at,
    })
}

async fn fetch_members(
    pool: &DbPool,
    group_id: &GroupId,
) -> Result<Vec<ModelGroupMember>, AppError> {
    let rows = sqlx::query_as::<_, ModelGroupMemberRow>(
        "SELECT job_id, model_name \
         FROM model_group_members \
         WHERE group_id = ? \
         ORDER BY id ASC",
    )
    .bind(group_id.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ModelGroupMember {
            job_id: r.job_id,
            model_name: r.model_name,
        })
        .collect())
}

/// Inserts a new model group record.
#[instrument(skip(pool), fields(group_id = %group.id, group_name = %group.name))]
pub async fn insert_group(pool: &DbPool, group: &ModelGroup) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO model_groups (id, name, dir_path, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(group.id.to_string())
    .bind(&group.name)
    .bind(group.dir_path.to_string_lossy().as_ref())
    .bind(group.created_at.to_rfc3339())
    .bind(group.updated_at.to_rfc3339())
    .execute(pool)
    .await?;

    tracing::info!("model group inserted");
    Ok(())
}

/// Returns all model groups ordered by creation time (newest first).
#[instrument(skip(pool))]
pub async fn list_groups(pool: &DbPool) -> Result<Vec<ModelGroup>, AppError> {
    let rows = sqlx::query_as::<_, ModelGroupRow>(
        "SELECT id, name, dir_path, created_at, updated_at \
         FROM model_groups \
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    let mut groups = Vec::with_capacity(rows.len());
    for row in rows {
        let gid: GroupId = row
            .id
            .parse()
            .map_err(|e| AppError::Validation(format!("invalid group id: {e}")))?;
        let members = fetch_members(pool, &gid).await?;
        groups.push(row_to_group(
            ModelGroupRow {
                id: gid.to_string(),
                ..row
            },
            members,
        )?);
    }
    Ok(groups)
}

/// Fetches a single model group by ID.
#[instrument(skip(pool), fields(group_id = %group_id))]
pub async fn get_group(pool: &DbPool, group_id: &GroupId) -> Result<ModelGroup, AppError> {
    let row = sqlx::query_as::<_, ModelGroupRow>(
        "SELECT id, name, dir_path, created_at, updated_at \
         FROM model_groups WHERE id = ?",
    )
    .bind(group_id.to_string())
    .fetch_one(pool)
    .await?;

    let members = fetch_members(pool, group_id).await?;
    row_to_group(row, members)
}

/// Updates the name of an existing group.
#[instrument(skip(pool), fields(group_id = %group_id, name))]
pub async fn update_group_name(
    pool: &DbPool,
    group_id: &GroupId,
    name: &str,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE model_groups SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(&now)
        .bind(group_id.to_string())
        .execute(pool)
        .await?;

    tracing::debug!("group name updated");
    Ok(())
}

/// Adds a member to a group. Silently ignores duplicate (same group_id + model_name).
#[instrument(skip(pool), fields(group_id = %group_id, model_name = %member.model_name))]
pub async fn add_group_member(
    pool: &DbPool,
    group_id: &GroupId,
    member: &ModelGroupMember,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO model_group_members \
         (group_id, job_id, model_name, created_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(group_id.to_string())
    .bind(&member.job_id)
    .bind(&member.model_name)
    .bind(&now)
    .execute(pool)
    .await?;

    tracing::debug!("group member added");
    Ok(())
}

/// Removes a member from a group by model name.
#[instrument(skip(pool), fields(group_id = %group_id, model_name))]
pub async fn remove_group_member(
    pool: &DbPool,
    group_id: &GroupId,
    model_name: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM model_group_members WHERE group_id = ? AND model_name = ?")
        .bind(group_id.to_string())
        .bind(model_name)
        .execute(pool)
        .await?;

    tracing::debug!("group member removed");
    Ok(())
}

/// Deletes a model group and all its members (cascade).
#[instrument(skip(pool), fields(group_id = %group_id))]
pub async fn delete_group(pool: &DbPool, group_id: &GroupId) -> Result<(), AppError> {
    sqlx::query("DELETE FROM model_groups WHERE id = ?")
        .bind(group_id.to_string())
        .execute(pool)
        .await?;

    tracing::info!("model group deleted");
    Ok(())
}

// ── Tritonserver serving CRUD ────────────────────────────────────────────────

#[derive(Debug, FromRow)]
struct ServingContainerRow {
    group_id: String,
    container_id: String,
    container_name: String,
    image_tag: String,
    gpu_id: i64,
    status: String,
    error_message: Option<String>,
    started_at: String,
    stopped_at: Option<String>,
}

fn row_to_serving(row: ServingContainerRow) -> Result<ServingContainer, AppError> {
    let group_id: GroupId = row
        .group_id
        .parse()
        .map_err(|e| AppError::Validation(format!("invalid serving group_id: {e}")))?;
    let gpu_id = u32::try_from(row.gpu_id)
        .map_err(|_| AppError::Validation("serving gpu_id out of u32 range".into()))?;
    let started_at = DateTime::parse_from_rfc3339(&row.started_at)
        .map_err(|e| AppError::Validation(format!("invalid serving started_at: {e}")))?
        .to_utc();
    let stopped_at = row
        .stopped_at
        .map(|raw| {
            DateTime::parse_from_rfc3339(&raw)
                .map(|dt| dt.to_utc())
                .map_err(|e| AppError::Validation(format!("invalid serving stopped_at: {e}")))
        })
        .transpose()?;

    Ok(ServingContainer {
        group_id,
        container_id: row.container_id,
        container_name: row.container_name,
        image_tag: row.image_tag,
        gpu_id,
        status: row.status.parse()?,
        error_message: row.error_message,
        started_at,
        stopped_at,
    })
}

/// Inserts or replaces the serving container row for a group.
#[instrument(skip(pool, container), fields(group_id = %container.group_id))]
pub async fn upsert_serving_container(
    pool: &DbPool,
    container: &ServingContainer,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO tritonserver_containers \
         (group_id, container_id, container_name, image_tag, gpu_id, status, \
          error_message, started_at, stopped_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(group_id) DO UPDATE SET \
            container_id = excluded.container_id, \
            container_name = excluded.container_name, \
            image_tag = excluded.image_tag, \
            gpu_id = excluded.gpu_id, \
            status = excluded.status, \
            error_message = excluded.error_message, \
            started_at = excluded.started_at, \
            stopped_at = excluded.stopped_at",
    )
    .bind(container.group_id.to_string())
    .bind(&container.container_id)
    .bind(&container.container_name)
    .bind(&container.image_tag)
    .bind(i64::from(container.gpu_id))
    .bind(container.status.to_string())
    .bind(&container.error_message)
    .bind(container.started_at.to_rfc3339())
    .bind(container.stopped_at.map(|dt| dt.to_rfc3339()))
    .execute(pool)
    .await?;

    tracing::info!(status = %container.status, "serving container row upserted");
    Ok(())
}

/// Updates only the status (and optional error/stopped timestamp) of a serving row.
#[instrument(skip(pool), fields(group_id = %group_id, %status))]
pub async fn update_serving_status(
    pool: &DbPool,
    group_id: &GroupId,
    status: ServingStatus,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let stopped_at =
        matches!(status, ServingStatus::Stopped | ServingStatus::Error).then_some(now.clone());

    sqlx::query(
        "UPDATE tritonserver_containers \
         SET status = ?, error_message = ?, stopped_at = COALESCE(?, stopped_at) \
         WHERE group_id = ?",
    )
    .bind(status.to_string())
    .bind(error_message)
    .bind(stopped_at)
    .bind(group_id.to_string())
    .execute(pool)
    .await?;

    Ok(())
}

/// Returns the serving container row for a group, if one exists.
#[instrument(skip(pool), fields(group_id = %group_id))]
pub async fn get_serving_by_group(
    pool: &DbPool,
    group_id: &GroupId,
) -> Result<Option<ServingContainer>, AppError> {
    let row = sqlx::query_as::<_, ServingContainerRow>(
        "SELECT group_id, container_id, container_name, image_tag, gpu_id, status, \
                error_message, started_at, stopped_at \
         FROM tritonserver_containers WHERE group_id = ?",
    )
    .bind(group_id.to_string())
    .fetch_optional(pool)
    .await?;

    row.map(row_to_serving).transpose()
}

/// Appends a batch of `tritonserver` log lines for a container.
#[instrument(skip(pool, logs), fields(container_id, count = logs.len()))]
pub async fn append_serving_logs_batch(
    pool: &DbPool,
    container_id: &str,
    logs: &[NewJobLog],
) -> Result<(), AppError> {
    if logs.is_empty() {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;

    for log in logs {
        sqlx::query(
            "INSERT INTO tritonserver_logs (container_id, stream, message, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(container_id)
        .bind(&log.stream)
        .bind(&log.message)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Returns the most recent persisted log lines for a serving container.
#[instrument(skip(pool), fields(container_id, limit))]
pub async fn tail_serving_logs(
    pool: &DbPool,
    container_id: &str,
    limit: u32,
) -> Result<Vec<String>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT message FROM tritonserver_logs \
         WHERE container_id = ? \
         ORDER BY id DESC \
         LIMIT ?",
    )
    .bind(container_id)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().rev().map(|(line,)| line).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_job() -> ConversionJob {
        let now = chrono::Utc::now();
        ConversionJob {
            id: JobId::new(),
            model_name: "resnet".to_string(),
            model_version: 1,
            model_format: ModelFormat::Onnx,
            image_tag: "nvcr.io/nvidia/tensorrt:24.08-py3".to_string(),
            gpu_id: GpuId(0),
            trt_options: TrtOptions::default(),
            status: JobStatus::Pending,
            progress_percent: 0,
            output_path: None,
            error_message: None,
            warmup_inputs: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn init_db_creates_missing_parent_directory() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("nested").join("converter.db");
        let database_url = format!("sqlite://{}", db_path.display());

        let pool = init_db(&database_url).await.expect("database init");
        pool.close().await;

        assert!(db_path.exists());
    }

    #[tokio::test]
    async fn init_db_uses_delete_journal_for_file_database() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("converter.db");
        let database_url = format!("sqlite://{}", db_path.display());

        let pool = init_db(&database_url).await.expect("database init");
        let journal_mode: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .expect("journal mode lookup");
        pool.close().await;

        assert_eq!(journal_mode.0, "delete");
    }

    #[tokio::test]
    async fn log_rows_insert_and_list_in_chronological_order() {
        let pool = init_db("sqlite::memory:").await.expect("database init");
        let job = sample_job();
        insert_job(&pool, &job).await.expect("insert job");

        append_job_logs_batch(
            &pool,
            &job.id,
            &[
                NewJobLog::new("stdout", "first"),
                NewJobLog::new("stderr", "second"),
                NewJobLog::new("stdout", "third"),
            ],
        )
        .await
        .expect("insert logs");

        let logs = list_job_logs(&pool, &job.id, 2).await.expect("list logs");
        let messages: Vec<_> = logs.iter().map(|log| log.message.as_str()).collect();

        assert_eq!(messages, vec!["second", "third"]);
        assert_eq!(logs[0].stream, "stderr");
        pool.close().await;
    }

    #[tokio::test]
    async fn logs_remain_queryable_after_job_completion() {
        let pool = init_db("sqlite::memory:").await.expect("database init");
        let job = sample_job();
        insert_job(&pool, &job).await.expect("insert job");
        append_job_logs_batch(&pool, &job.id, &[NewJobLog::new("stdout", "done")])
            .await
            .expect("insert log");

        update_job_completed(&pool, &job.id, std::path::Path::new("/tmp/model"))
            .await
            .expect("complete job");

        let logs = list_job_logs(&pool, &job.id, 100).await.expect("list logs");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "done");
        pool.close().await;
    }

    #[tokio::test]
    async fn delete_job_helpers_remove_logs_and_job_row() {
        let pool = init_db("sqlite::memory:").await.expect("database init");
        let job = sample_job();
        insert_job(&pool, &job).await.expect("insert job");
        append_job_logs_batch(&pool, &job.id, &[NewJobLog::new("stdout", "done")])
            .await
            .expect("insert log");

        delete_job_logs(&pool, &job.id).await.expect("delete logs");
        delete_job(&pool, &job.id).await.expect("delete job");

        let logs = list_job_logs(&pool, &job.id, 100).await.expect("list logs");
        let jobs = list_jobs(&pool, 20, 0).await.expect("list jobs");

        assert!(logs.is_empty());
        assert!(jobs.is_empty());
        pool.close().await;
    }

    #[tokio::test]
    async fn migration_creates_conversion_job_logs_table() {
        let pool = init_db("sqlite::memory:").await.expect("database init");
        let table_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name = 'conversion_job_logs'",
        )
        .fetch_one(&pool)
        .await
        .expect("table lookup");

        assert_eq!(table_count.0, 1);
        pool.close().await;
    }
}
