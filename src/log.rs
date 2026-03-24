use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init() -> tracing_appender::non_blocking::WorkerGuard {
    let home = std::env::var("HOME").expect("HOME environment variable not found.");
    let log_dir = PathBuf::from(home).join(".cache").join("shuttle");
    std::fs::create_dir_all(&log_dir).unwrap_or_else(|_| {
        eprintln!("Failed to create log directory at {:?}", log_dir);
    });

    let file_appender = tracing_appender::rolling::daily(&log_dir, "shuttle.log");

    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,wayland_client=warn,calloop=warn"));

    let stdout_layer = fmt::layer()
        .with_ansi(true)
        .with_target(false)
        .without_time();

    let file_layer = fmt::layer()
        .with_writer(non_blocking_appender)
        .with_ansi(false)
        .with_thread_ids(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    info!("Logging engine initialized. Log file path: {:?}", log_dir);
    guard
}
