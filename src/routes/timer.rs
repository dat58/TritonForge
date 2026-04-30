//! Cross-target async timers for route polling loops.

use std::time::Duration;

/// Sleeps for `duration` without using unsupported WASM time APIs.
pub(crate) async fn sleep(duration: Duration) {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::sleep(duration).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        futures_timer::Delay::new(duration).await;
    }
}
