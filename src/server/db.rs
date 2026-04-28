//! SQLite persistence layer: connection pool initialisation and CRUD helpers.

use crate::errors::AppError;
use crate::models::config::GpuId;
use crate::models::job::{ConversionJob, JobId, JobStatus, ModelFormat, TrtOptions};
use chrono::DateTime;
use sqlx::{FromRow, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;
use tracing::instrument;

/// Shared connection pool alias.
pub type DbPool = SqlitePool;

/// Connects to the SQLite database and runs any pending migrations.
///
/// `database_url` must be a valid SQLite URL such as `sqlite://data/converter.db`
/// or the special `sqlite::memory:` for in-memory testing.
#[instrument(skip_all, fields(database_url))]
pub async fn init_db(database_url: &str) -> Result<DbPool, AppError> {
    let pool = SqlitePool::connect(database_url).await?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(|e| AppError::Conversion(format!("migration failed: {e}")))?;
    tracing::info!(database_url, "database pool initialised");
    Ok(pool)
}

/// Raw row returned by SQLite queries — converted to [`ConversionJob`] before leaving this module.
#[derive(Debug, FromRow)]
struct ConversionJobRow {
    id: String,
    model_name: String,
    model_format: String,
    image_tag: String,
    gpu_id: i64,
    template_name: String,
    trt_options: String,
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

    let created_at = DateTime::parse_from_rfc3339(&row.created_at)
        .map_err(|e| AppError::Validation(format!("invalid created_at: {e}")))?
        .to_utc();

    let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
        .map_err(|e| AppError::Validation(format!("invalid updated_at: {e}")))?
        .to_utc();

    let trt_options: TrtOptions = serde_json::from_str(&row.trt_options)
        .map_err(|e| AppError::Conversion(format!("failed to parse trt_options: {e}")))?;

    Ok(ConversionJob {
        id: JobId(id_uuid),
        model_name: row.model_name,
        model_format: ModelFormat::from_str(&row.model_format)?,
        image_tag: row.image_tag,
        gpu_id: GpuId(gpu_raw),
        template_name: row.template_name,
        trt_options,
        status: JobStatus::from_str(&row.status)?,
        progress_percent: u8::try_from(row.progress_percent).unwrap_or(100),
        output_path: row.output_path.map(PathBuf::from),
        error_message: row.error_message,
        created_at,
        updated_at,
    })
}

/// Inserts a new job record into the database.
#[instrument(skip(pool), fields(job_id = %job.id))]
pub async fn insert_job(pool: &DbPool, job: &ConversionJob) -> Result<(), AppError> {
    let trt_options_json = serde_json::to_string(&job.trt_options)
        .map_err(|e| AppError::Conversion(format!("failed to serialize trt_options: {e}")))?;

    sqlx::query(
        "INSERT INTO conversion_jobs \
         (id, model_name, model_format, image_tag, gpu_id, template_name, trt_options, \
          status, progress_percent, output_path, error_message, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job.id.to_string())
    .bind(&job.model_name)
    .bind(job.model_format.to_string())
    .bind(&job.image_tag)
    .bind(i64::from(job.gpu_id.0))
    .bind(&job.template_name)
    .bind(trt_options_json)
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
        "SELECT id, model_name, model_format, image_tag, gpu_id, template_name, trt_options, \
         status, progress_percent, output_path, error_message, created_at, updated_at \
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
        "SELECT id, model_name, model_format, image_tag, gpu_id, template_name, trt_options, \
         status, progress_percent, output_path, error_message, created_at, updated_at \
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
