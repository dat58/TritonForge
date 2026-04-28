//! Server-side modules: Docker, GPU detection, database, storage, conversion pipeline,
//! and Dioxus server functions.
//!
//! All items in this module are compiled only for native targets (not WASM).

pub mod conversion;
pub mod db;
pub mod docker;
pub mod gpu;
pub mod onnx_config;
pub mod storage;
