//! File-system storage service: upload staging, output management, and cleanup.

use crate::errors::AppError;
use crate::models::config::AppConfig;
use crate::models::job::JobId;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::instrument;
use uuid::Uuid;

const ALLOWED_EXTENSIONS: &[&str] = &["onnx", "pb", "savedmodel"];

/// Manages all on-disk file operations for conversion jobs.
#[derive(Debug, Clone)]
pub struct StorageService {
    upload_dir: PathBuf,
    output_dir: PathBuf,
    max_upload_bytes: u64,
}

impl StorageService {
    /// Returns the configured upload staging directory.
    pub fn upload_dir(&self) -> &Path {
        &self.upload_dir
    }

    /// Creates a new `StorageService` from the application configuration.
    pub fn new(config: &AppConfig) -> Self {
        Self {
            upload_dir: config.upload_dir.clone(),
            output_dir: config.output_dir.clone(),
            max_upload_bytes: config.max_upload_size_mb * 1024 * 1024,
        }
    }

    /// Persists an uploaded model to the staging directory.
    ///
    /// The file is saved under a UUID-based name preserving the original extension.
    /// Returns the saved path and the file size in bytes.
    #[instrument(skip(self, bytes), fields(filename, file_size = bytes.len()))]
    pub async fn save_upload(
        &self,
        filename: &str,
        bytes: &[u8],
    ) -> Result<(PathBuf, u64), AppError> {
        validate_extension(filename)?;

        let file_size = bytes.len() as u64;
        if file_size > self.max_upload_bytes {
            return Err(AppError::Validation(format!(
                "file size {file_size} bytes exceeds maximum {} bytes",
                self.max_upload_bytes
            )));
        }

        fs::create_dir_all(&self.upload_dir).await?;

        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");

        let saved_name = format!("{}.{ext}", Uuid::new_v4());
        let saved_path = self.upload_dir.join(&saved_name);

        fs::write(&saved_path, bytes).await?;
        tracing::info!(file_size, path = %saved_path.display(), "upload saved");

        Ok((saved_path, file_size))
    }

    /// Moves a completed engine file from temp storage into the output directory.
    ///
    /// Output is organised as `output_dir/{job_id}/{model_name}.engine`.
    #[instrument(skip(self), fields(job_id = %job_id, model_name))]
    pub async fn move_to_output(
        &self,
        temp_path: &Path,
        job_id: &JobId,
        model_name: &str,
    ) -> Result<PathBuf, AppError> {
        let job_dir = self.output_dir.join(job_id.to_string());
        fs::create_dir_all(&job_dir).await?;

        let dest = job_dir.join(format!("{model_name}.engine"));
        fs::rename(temp_path, &dest).await?;

        tracing::info!(dest = %dest.display(), "engine moved to output directory");
        Ok(dest)
    }

    /// Copies the completed engine to a user-specified server path.
    #[instrument(skip(self), fields(target = %target_path.display()))]
    pub async fn save_to_server_path(
        &self,
        source: &Path,
        target_path: &Path,
    ) -> Result<PathBuf, AppError> {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::copy(source, target_path).await?;
        tracing::info!(dest = %target_path.display(), "engine copied to server path");
        Ok(target_path.to_owned())
    }

    /// Returns the path of the `.engine` file for the given job, if it exists.
    #[instrument(skip(self), fields(job_id = %job_id))]
    pub async fn get_download_path(&self, job_id: &JobId) -> Result<PathBuf, AppError> {
        let job_dir = self.output_dir.join(job_id.to_string());
        let mut entries = fs::read_dir(&job_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("engine") {
                return Ok(path);
            }
        }

        Err(AppError::Validation(format!(
            "no engine file found for job {job_id}"
        )))
    }

    /// Deletes a temporary file, ignoring the error if the file is already gone.
    #[instrument(skip(self), fields(path = %path.display()))]
    pub async fn cleanup_temp(&self, path: &Path) -> Result<(), AppError> {
        match fs::remove_file(path).await {
            Ok(()) => {
                tracing::debug!("temp file cleaned up");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

fn validate_extension(filename: &str) -> Result<(), AppError> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ALLOWED_EXTENSIONS.contains(&ext) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "unsupported file extension '.{ext}'; allowed: {ALLOWED_EXTENSIONS:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_extension_accepts_onnx() {
        assert!(validate_extension("model.onnx").is_ok());
    }

    #[test]
    fn validate_extension_rejects_unknown() {
        assert!(validate_extension("model.h5").is_err());
    }
}
