//! Shared Tokio runtime for network and background work.

use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};

static RT: OnceLock<Runtime> = OnceLock::new();

pub fn rt() -> &'static Runtime {
    RT.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("muninn-net")
            .enable_all()
            .build()
            .expect("failed to start Tokio runtime")
    })
}
