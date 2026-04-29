//! TensorRT Converter — shared library for all application modules.
//!
//! Both the server binary and WASM client are compiled from this library.
//! Server-only modules are gated with `#[cfg(not(target_arch = "wasm32"))]`.

pub mod api;
pub mod app;
pub mod components;
pub mod errors;
pub mod models;
pub mod onnx;
pub mod routes;

#[cfg(not(target_arch = "wasm32"))]
pub mod server;
