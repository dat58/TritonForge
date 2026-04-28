fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    tensorrt_converter::models::config::load_dotenv();

    #[cfg(not(target_arch = "wasm32"))]
    init_tracing();

    #[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
    dioxus::LaunchBuilder::server().launch(tensorrt_converter::app::App);

    #[cfg(any(target_arch = "wasm32", not(feature = "server")))]
    dioxus::launch(tensorrt_converter::app::App);
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
