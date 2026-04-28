fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    tensorrt_converter::models::config::load_dotenv();

    #[cfg(not(target_arch = "wasm32"))]
    init_tracing();

    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    launch_server();

    #[cfg(any(target_arch = "wasm32", not(feature = "server")))]
    dioxus::launch(tensorrt_converter::app::App);
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn launch_server() -> ! {
    use dioxus::prelude::DioxusRouterExt;
    use dioxus::server::ServeConfig;
    use dioxus::server::axum::{Router, extract::DefaultBodyLimit};
    use tensorrt_converter::models::config::AppConfig;

    let upload_limit = upload_limit_bytes(&AppConfig::from_env());

    dioxus::server::serve(move || async move {
        Ok(Router::new()
            .serve_dioxus_application(ServeConfig::new(), tensorrt_converter::app::App)
            .layer(DefaultBodyLimit::max(upload_limit)))
    });
}

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
fn upload_limit_bytes(config: &tensorrt_converter::models::config::AppConfig) -> usize {
    let bytes = config.max_upload_size_mb.saturating_mul(1024 * 1024);
    usize::try_from(bytes).unwrap_or(usize::MAX)
}

#[cfg(not(target_arch = "wasm32"))]
fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .init();
}
