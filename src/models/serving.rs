//! Domain types for tritonserver containers serving a model group.

use crate::errors::AppError;
use crate::models::group::GroupId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Host ports published for the Triton HTTP, gRPC, and metrics endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServingPortBindings {
    /// Host port forwarded to container port 8000/tcp.
    pub http: u16,
    /// Host port forwarded to container port 8001/tcp.
    pub grpc: u16,
    /// Host port forwarded to container port 8002/tcp.
    pub metrics: u16,
}

impl Default for ServingPortBindings {
    fn default() -> Self {
        Self {
            http: 8000,
            grpc: 8001,
            metrics: 8002,
        }
    }
}

/// User-selected Docker runtime options for a group serving container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartServingOptions {
    /// GPU device index passed to Docker's GPU device request.
    pub gpu_id: u32,
    /// Host ports used for Docker `-p host:container` publishing.
    pub ports: ServingPortBindings,
    /// Docker network mode/name passed as Docker `--net`.
    pub network: String,
}

/// Lifecycle states for a `tritonserver` container managed by TritonForge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServingStatus {
    /// Container created and start command issued; not yet confirmed running.
    Starting,
    /// Container is running.
    Running,
    /// Container has been stopped (gracefully or by user).
    Stopped,
    /// Container failed to start or exited with an error.
    Error,
}

impl std::fmt::Display for ServingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for ServingStatus {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopped" => Ok(Self::Stopped),
            "error" => Ok(Self::Error),
            other => Err(AppError::Validation(format!(
                "unknown serving status: {other}"
            ))),
        }
    }
}

/// Persisted record describing a tritonserver container managed for a group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServingContainer {
    /// Group this container is serving.
    pub group_id: GroupId,
    /// Docker-assigned container ID.
    pub container_id: String,
    /// User-visible container name (e.g. `tritonforge-serve-{group_id}`).
    pub container_name: String,
    /// Full image tag used to launch the container.
    pub image_tag: String,
    /// GPU device index passed to the container.
    pub gpu_id: u32,
    /// Current lifecycle state.
    pub status: ServingStatus,
    /// Optional human-readable error message when `status == Error`.
    pub error_message: Option<String>,
    /// When the container was last (re)started.
    pub started_at: DateTime<Utc>,
    /// When the container was last stopped (`None` while running).
    pub stopped_at: Option<DateTime<Utc>>,
}
