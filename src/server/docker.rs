//! Docker daemon integration via the Bollard async client.

use crate::errors::AppError;
use crate::models::config::TensorRtImage;
use bollard::Docker;
use bollard::models::ImageSummary;
use bollard::query_parameters::{CreateImageOptionsBuilder, ListImagesOptionsBuilder};
use std::collections::HashMap;
use tokio_stream::StreamExt;
use tracing::instrument;

const TENSORRT_IMAGE_PREFIX: &str = "nvcr.io/nvidia/tensorrt";

/// Async wrapper around the Bollard Docker client.
#[derive(Clone, Debug)]
pub struct DockerService {
    client: Docker,
}

impl DockerService {
    /// Connects to the Docker daemon using default socket paths.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let service = tensorrt_converter::server::docker::DockerService::new().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument]
    pub async fn new() -> Result<Self, AppError> {
        let client = Docker::connect_with_local_defaults()?;
        client.ping().await?;
        tracing::info!("connected to Docker daemon");
        Ok(Self { client })
    }

    /// Returns TensorRT images already present in the local Docker image cache.
    #[instrument(skip(self))]
    pub async fn list_tensorrt_images(&self) -> Result<Vec<TensorRtImage>, AppError> {
        let mut filters: HashMap<&str, Vec<&str>> = HashMap::new();
        filters.insert("reference", vec![TENSORRT_IMAGE_PREFIX]);

        let options = ListImagesOptionsBuilder::default()
            .filters(&filters)
            .build();

        let summaries = self.client.list_images(Some(options)).await?;
        let images: Vec<TensorRtImage> = summaries
            .into_iter()
            .filter_map(parse_tensorrt_image)
            .collect();

        tracing::debug!(count = images.len(), "listed TensorRT images");
        Ok(images)
    }

    /// Returns `true` if the given image tag is available locally.
    #[instrument(skip(self), fields(tag = %tag))]
    pub async fn is_image_available(&self, tag: &str) -> Result<bool, AppError> {
        let mut filters: HashMap<&str, Vec<&str>> = HashMap::new();
        filters.insert("reference", vec![tag]);

        let options = ListImagesOptionsBuilder::default()
            .filters(&filters)
            .build();

        let images = self.client.list_images(Some(options)).await?;
        Ok(!images.is_empty())
    }

    /// Pulls `tag` from the registry, streaming pull progress.
    ///
    /// Resolves when the pull completes or errors on the first stream error.
    #[instrument(skip(self), fields(tag = %tag))]
    pub async fn pull_image(&self, tag: &str) -> Result<(), AppError> {
        let options = CreateImageOptionsBuilder::default().from_image(tag).build();

        let mut stream = self.client.create_image(Some(options), None, None);

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    tracing::trace!(status = ?info.status, "pull progress");
                }
                Err(err) => {
                    tracing::error!(error = ?err, tag, "image pull failed");
                    return Err(err.into());
                }
            }
        }

        tracing::info!(tag, "image pull complete");
        Ok(())
    }
}

fn parse_tensorrt_image(summary: ImageSummary) -> Option<TensorRtImage> {
    let tag = summary.repo_tags.into_iter().next()?;
    let version_part = tag
        .strip_prefix(&format!("{TENSORRT_IMAGE_PREFIX}:"))
        .map(str::to_owned)?;

    Some(TensorRtImage {
        name: format!("TensorRT {version_part}"),
        tensorrt_version: version_part,
        cuda_version: String::new(),
        tag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn docker_service_new_connects_or_errors_gracefully() {
        // Requires a running Docker daemon; verifies the error path is correct.
        match DockerService::new().await {
            Ok(_) => {}
            Err(AppError::Docker(_)) => {}
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}
