/// Integration tests for DockerService.
///
/// These tests require a running Docker daemon. They are automatically skipped
/// when the Docker socket is unavailable.
#[cfg(test)]
mod docker_integration {
    fn docker_available() -> bool {
        std::path::Path::new("/var/run/docker.sock").exists()
    }

    /// Verify that DockerService can connect to the local Docker daemon.
    #[tokio::test]
    async fn connects_to_docker_daemon() {
        if !docker_available() {
            eprintln!("Skipping: Docker socket not found");
            return;
        }

        let result = tensorrt_converter::server::docker::DockerService::new().await;
        assert!(
            result.is_ok(),
            "DockerService::new() failed: {:?}",
            result.err()
        );
    }

    /// Verify that listing TensorRT images does not panic when Docker is available.
    #[tokio::test]
    async fn list_images_returns_vec() {
        if !docker_available() {
            eprintln!("Skipping: Docker socket not found");
            return;
        }

        let svc = tensorrt_converter::server::docker::DockerService::new()
            .await
            .expect("DockerService::new()");

        let result = svc.list_tensorrt_images().await;
        assert!(
            result.is_ok(),
            "list_tensorrt_images() failed: {:?}",
            result.err()
        );
    }
}
