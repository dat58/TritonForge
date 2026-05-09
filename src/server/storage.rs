//! File-system storage service: upload staging, output management, and cleanup.

use crate::errors::AppError;
use crate::models::config::AppConfig;
use crate::models::job::JobId;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::instrument;
use uuid::Uuid;

const ALLOWED_EXTENSIONS: &[&str] = &["onnx"];
const ZIP_LOCAL_FILE_HEADER: u32 = 0x0403_4b50;
const ZIP_CENTRAL_DIRECTORY_HEADER: u32 = 0x0201_4b50;
const ZIP_END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;

/// Manages all on-disk file operations for conversion jobs and model groups.
#[derive(Debug, Clone)]
pub struct StorageService {
    upload_dir: PathBuf,
    output_dir: PathBuf,
    groups_dir: PathBuf,
    max_upload_bytes: u64,
}

impl StorageService {
    /// Returns the configured upload staging directory.
    pub fn upload_dir(&self) -> &Path {
        &self.upload_dir
    }

    /// Returns the configured model groups root directory.
    pub fn groups_dir(&self) -> &Path {
        &self.groups_dir
    }

    /// Creates a new `StorageService` from the application configuration.
    pub fn new(config: &AppConfig) -> Self {
        Self {
            upload_dir: config.upload_dir.clone(),
            output_dir: config.output_dir.clone(),
            groups_dir: config.groups_dir.clone(),
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

    /// Copies an existing server-side ONNX model into the upload staging directory.
    ///
    /// The source path must point to a readable regular `.onnx` file and must fit
    /// within the configured upload size limit.
    #[instrument(skip(self), fields(source_path = %source_path.display()))]
    pub async fn copy_server_model_to_uploads(
        &self,
        source_path: &Path,
    ) -> Result<(PathBuf, u64), AppError> {
        validate_server_model_path(source_path).await?;
        let file_size = validate_file_size(source_path, self.max_upload_bytes).await?;

        fs::create_dir_all(&self.upload_dir).await?;
        let saved_path = self.upload_dir.join(format!("{}.onnx", Uuid::new_v4()));
        fs::copy(source_path, &saved_path).await?;

        tracing::info!(
            file_size,
            source = %source_path.display(),
            path = %saved_path.display(),
            "server model copied to uploads"
        );

        Ok((saved_path, file_size))
    }

    /// Reads an existing server-side ONNX model after applying upload validation.
    #[instrument(skip(self), fields(source_path = %source_path.display()))]
    pub async fn read_server_model(&self, source_path: &Path) -> Result<Vec<u8>, AppError> {
        validate_server_model_path(source_path).await?;
        validate_file_size(source_path, self.max_upload_bytes).await?;
        fs::read(source_path).await.map_err(AppError::Io)
    }

    /// Moves a completed plan file into Triton's model repository layout.
    ///
    /// Output is organised as `output_dir/{job_id}/{model_name}/{version}/model.plan`.
    #[instrument(skip(self, config_pbtxt), fields(job_id = %job_id, model_name, model_version))]
    pub async fn move_to_output(
        &self,
        temp_path: &Path,
        job_id: &JobId,
        model_name: &str,
        model_version: u32,
        config_pbtxt: &str,
    ) -> Result<PathBuf, AppError> {
        let model_dir = self.output_dir.join(job_id.to_string()).join(model_name);
        let version_dir = model_dir.join(model_version.to_string());
        fs::create_dir_all(&version_dir).await?;

        let plan_path = version_dir.join("model.plan");
        fs::copy(temp_path, &plan_path).await?;
        fs::write(model_dir.join("config.pbtxt"), config_pbtxt).await?;

        tracing::info!(dest = %model_dir.display(), "model moved to output directory");
        Ok(model_dir)
    }

    /// Returns the path of the Triton model directory for the given job.
    #[instrument(skip(self), fields(job_id = %job_id))]
    pub async fn get_model_dir(
        &self,
        job_id: &JobId,
        model_name: &str,
    ) -> Result<PathBuf, AppError> {
        let model_dir = self.output_dir.join(job_id.to_string()).join(model_name);
        if fs::metadata(&model_dir).await?.is_dir() {
            return Ok(model_dir);
        }

        Err(AppError::Validation(format!(
            "no model directory found for job {job_id}"
        )))
    }

    /// Reads the rendered `config.pbtxt` for a completed job's model.
    #[instrument(skip(self), fields(job_id = %job_id, model_name))]
    pub async fn read_config_pbtxt(
        &self,
        job_id: &JobId,
        model_name: &str,
    ) -> Result<String, AppError> {
        let path = self.config_pbtxt_path(job_id, model_name);
        match fs::read_to_string(&path).await {
            Ok(contents) => Ok(contents),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AppError::Validation(
                format!("config.pbtxt not found for job {job_id}"),
            )),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomically overwrites the `config.pbtxt` for a completed job's model.
    ///
    /// Writes to a sibling temp file then renames over the destination so an
    /// interrupted write cannot truncate the existing file.
    #[instrument(skip(self, contents), fields(job_id = %job_id, model_name, byte_len = contents.len()))]
    pub async fn write_config_pbtxt(
        &self,
        job_id: &JobId,
        model_name: &str,
        contents: &str,
    ) -> Result<(), AppError> {
        let dest = self.config_pbtxt_path(job_id, model_name);
        let model_dir = dest
            .parent()
            .ok_or_else(|| AppError::Validation(format!("no model directory for job {job_id}")))?;

        if !fs::metadata(model_dir).await?.is_dir() {
            return Err(AppError::Validation(format!(
                "no model directory found for job {job_id}"
            )));
        }

        let tmp = model_dir.join(format!(".config.pbtxt.{}.tmp", Uuid::new_v4()));
        fs::write(&tmp, contents).await?;
        if let Err(e) = fs::rename(&tmp, &dest).await {
            let _ = fs::remove_file(&tmp).await;
            return Err(e.into());
        }

        tracing::info!(path = %dest.display(), "config.pbtxt updated");
        Ok(())
    }

    fn config_pbtxt_path(&self, job_id: &JobId, model_name: &str) -> PathBuf {
        self.output_dir
            .join(job_id.to_string())
            .join(model_name)
            .join("config.pbtxt")
    }

    /// Deletes a job's full output root directory (`output_dir/{job_id}/`).
    #[instrument(skip(self), fields(job_id = %job_id))]
    pub async fn delete_job_output_root(&self, job_id: &JobId) -> Result<(), AppError> {
        let dir = self.output_dir.join(job_id.to_string());
        match fs::remove_dir_all(&dir).await {
            Ok(()) => {
                tracing::info!(dir = %dir.display(), "job output root deleted");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns a zip archive containing the full Triton model directory.
    #[instrument(skip(self), fields(model_dir = %model_dir.display(), model_name))]
    pub async fn zip_model_dir(
        &self,
        model_dir: &Path,
        model_name: &str,
    ) -> Result<Vec<u8>, AppError> {
        let files = collect_model_files(model_dir, model_name).await?;
        build_zip(files)
    }

    /// Copies a model directory tree into a group directory.
    ///
    /// Source: `output_dir/{job_id}/{model_name}/`
    /// Destination: `groups_dir/{group_name}/{model_name}/`
    /// Returns the destination path.
    #[instrument(skip(self), fields(job_id, model_name, group_name))]
    pub async fn copy_model_to_group(
        &self,
        job_id: &str,
        model_name: &str,
        group_name: &str,
    ) -> Result<PathBuf, AppError> {
        let src = self.output_dir.join(job_id).join(model_name);
        let dst = self.groups_dir.join(group_name).join(model_name);
        copy_dir_all(&src, &dst).await?;
        tracing::info!(dest = %dst.display(), "model copied to group");
        Ok(dst)
    }

    /// Deletes the entire group directory (`groups_dir/{group_name}/`).
    #[instrument(skip(self), fields(group_name))]
    pub async fn delete_group_dir(&self, group_name: &str) -> Result<(), AppError> {
        let dir = self.groups_dir.join(group_name);
        match fs::remove_dir_all(&dir).await {
            Ok(()) => {
                tracing::info!(dir = %dir.display(), "group directory deleted");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Deletes a single copied model directory from a group.
    #[instrument(skip(self), fields(group_name, model_name))]
    pub async fn delete_group_model_dir(
        &self,
        group_name: &str,
        model_name: &str,
    ) -> Result<(), AppError> {
        let dir = self.groups_dir.join(group_name).join(model_name);
        match fs::remove_dir_all(&dir).await {
            Ok(()) => {
                tracing::info!(dir = %dir.display(), "group model directory deleted");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Deletes a single job's model output directory (`output_dir/{job_id}/{model_name}/`).
    #[instrument(skip(self), fields(job_id, model_name))]
    pub async fn delete_job_output_dir(
        &self,
        job_id: &str,
        model_name: &str,
    ) -> Result<(), AppError> {
        let dir = self.output_dir.join(job_id).join(model_name);
        match fs::remove_dir_all(&dir).await {
            Ok(()) => {
                tracing::info!(dir = %dir.display(), "job output directory deleted");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
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

async fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dst).await?;
    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let entry_dst = dst.join(entry.file_name());
        if entry.metadata().await?.is_dir() {
            Box::pin(copy_dir_all(&entry.path(), &entry_dst)).await?;
        } else {
            fs::copy(entry.path(), entry_dst).await?;
        }
    }
    Ok(())
}

fn validate_extension(filename: &str) -> Result<(), AppError> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "unsupported file extension '.{ext}'; allowed: {ALLOWED_EXTENSIONS:?}"
        )))
    }
}

async fn validate_server_model_path(source_path: &Path) -> Result<(), AppError> {
    validate_extension(source_path.to_string_lossy().as_ref())?;
    let metadata = fs::metadata(source_path).await?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "model path '{}' is not a regular file",
            source_path.display()
        )))
    }
}

async fn validate_file_size(source_path: &Path, max_upload_bytes: u64) -> Result<u64, AppError> {
    let file_size = fs::metadata(source_path).await?.len();
    if file_size <= max_upload_bytes {
        Ok(file_size)
    } else {
        Err(AppError::Validation(format!(
            "file size {file_size} bytes exceeds maximum {max_upload_bytes} bytes"
        )))
    }
}

async fn collect_model_files(
    model_dir: &Path,
    model_name: &str,
) -> Result<Vec<(String, Vec<u8>)>, AppError> {
    let mut dirs = vec![model_dir.to_owned()];
    let mut files = Vec::new();

    while let Some(dir) = dirs.pop() {
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                dirs.push(path);
            } else if metadata.is_file() {
                files.push(read_model_file(model_dir, model_name, &path).await?);
            }
        }
    }

    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

async fn read_model_file(
    model_dir: &Path,
    model_name: &str,
    path: &Path,
) -> Result<(String, Vec<u8>), AppError> {
    let relative = path
        .strip_prefix(model_dir)
        .map_err(|e| AppError::Conversion(format!("invalid model file path: {e}")))?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    let zip_name = format!("{model_name}/{relative}");
    let bytes = fs::read(path).await?;
    Ok((zip_name, bytes))
}

struct ZipCentralEntry {
    name: String,
    crc32: u32,
    size: u32,
    local_offset: u32,
}

fn build_zip(files: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, AppError> {
    let mut archive = Vec::new();
    let mut central_entries = Vec::new();

    for (name, bytes) in files {
        let local_offset = checked_len_u32(archive.len(), "zip offset")?;
        let crc32 = crc32(&bytes);
        let size = checked_len_u32(bytes.len(), "zip file size")?;
        write_local_file(&mut archive, &name, &bytes, crc32, size)?;
        central_entries.push(ZipCentralEntry {
            name,
            crc32,
            size,
            local_offset,
        });
    }

    let central_offset = checked_len_u32(archive.len(), "zip central offset")?;
    for entry in &central_entries {
        write_central_entry(&mut archive, entry)?;
    }
    let central_size = checked_len_u32(
        archive.len().saturating_sub(central_offset as usize),
        "zip central size",
    )?;
    write_zip_end(
        &mut archive,
        central_entries.len(),
        central_size,
        central_offset,
    )?;
    Ok(archive)
}

fn write_local_file(
    archive: &mut Vec<u8>,
    name: &str,
    bytes: &[u8],
    crc32: u32,
    size: u32,
) -> Result<(), AppError> {
    let name_len = checked_len_u16(name.len(), "zip file name")?;
    push_u32(archive, ZIP_LOCAL_FILE_HEADER);
    push_u16(archive, 20);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u32(archive, crc32);
    push_u32(archive, size);
    push_u32(archive, size);
    push_u16(archive, name_len);
    push_u16(archive, 0);
    archive.extend_from_slice(name.as_bytes());
    archive.extend_from_slice(bytes);
    Ok(())
}

fn write_central_entry(archive: &mut Vec<u8>, entry: &ZipCentralEntry) -> Result<(), AppError> {
    let name_len = checked_len_u16(entry.name.len(), "zip central file name")?;
    push_u32(archive, ZIP_CENTRAL_DIRECTORY_HEADER);
    push_u16(archive, 20);
    push_u16(archive, 20);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u32(archive, entry.crc32);
    push_u32(archive, entry.size);
    push_u32(archive, entry.size);
    push_u16(archive, name_len);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u32(archive, 0);
    push_u32(archive, entry.local_offset);
    archive.extend_from_slice(entry.name.as_bytes());
    Ok(())
}

fn write_zip_end(
    archive: &mut Vec<u8>,
    entries_len: usize,
    central_size: u32,
    central_offset: u32,
) -> Result<(), AppError> {
    let entries = checked_len_u16(entries_len, "zip entry count")?;
    push_u32(archive, ZIP_END_OF_CENTRAL_DIRECTORY);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, entries);
    push_u16(archive, entries);
    push_u32(archive, central_size);
    push_u32(archive, central_offset);
    push_u16(archive, 0);
    Ok(())
}

fn checked_len_u16(len: usize, label: &str) -> Result<u16, AppError> {
    u16::try_from(len).map_err(|_| AppError::Conversion(format!("{label} exceeds ZIP limits")))
}

fn checked_len_u32(len: usize, label: &str) -> Result<u32, AppError> {
    u32::try_from(len).map_err(|_| AppError::Conversion(format!("{label} exceeds ZIP limits")))
}

fn push_u16(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_extension_accepts_onnx() {
        assert!(validate_extension("model.onnx").is_ok());
        assert!(validate_extension("MODEL.ONNX").is_ok());
    }

    #[test]
    fn validate_extension_rejects_unknown() {
        assert!(validate_extension("model.h5").is_err());
    }

    #[test]
    fn validate_extension_rejects_savedmodel() {
        assert!(validate_extension("model.savedmodel").is_err());
    }

    #[tokio::test]
    async fn copy_server_model_to_uploads_copies_valid_onnx() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("MODEL.ONNX");
        tokio::fs::write(&source_path, b"onnx-bytes")
            .await
            .expect("write source");
        let storage = test_storage(temp.path(), 1);

        let (copied_path, file_size) = storage
            .copy_server_model_to_uploads(&source_path)
            .await
            .expect("copy server model");

        assert_eq!(file_size, 10);
        assert!(copied_path.starts_with(temp.path().join("uploads")));
        assert_eq!(
            tokio::fs::read(copied_path).await.expect("read copied"),
            b"onnx-bytes"
        );
    }

    #[tokio::test]
    async fn copy_server_model_to_uploads_rejects_invalid_extension() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("model.bin");
        tokio::fs::write(&source_path, b"onnx-bytes")
            .await
            .expect("write source");
        let storage = test_storage(temp.path(), 1);

        let result = storage.copy_server_model_to_uploads(&source_path).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn copy_server_model_to_uploads_rejects_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("model.onnx");
        tokio::fs::create_dir(&source_path)
            .await
            .expect("create source directory");
        let storage = test_storage(temp.path(), 1);

        let result = storage.copy_server_model_to_uploads(&source_path).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn copy_server_model_to_uploads_rejects_oversized_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source_path = temp.path().join("model.onnx");
        tokio::fs::write(&source_path, b"x")
            .await
            .expect("write source");
        let storage = test_storage(temp.path(), 0);

        let result = storage.copy_server_model_to_uploads(&source_path).await;

        assert!(result.is_err());
    }

    fn test_storage(root: &Path, max_upload_size_mb: u64) -> StorageService {
        StorageService::new(&AppConfig {
            upload_dir: root.join("uploads"),
            output_dir: root.join("outputs"),
            max_upload_size_mb,
            conversion_timeout_secs: 1800,
            docker_socket: PathBuf::from("/var/run/docker.sock"),
            groups_dir: root.join("groups"),
        })
    }
}
