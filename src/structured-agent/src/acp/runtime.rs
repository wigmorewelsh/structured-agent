use std::sync::LazyLock;

pub static AGENT_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("agent-worker")
        .enable_all()
        .build()
        .expect("Failed to create agent runtime")
});
