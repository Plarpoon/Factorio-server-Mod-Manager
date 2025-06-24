use tracing_subscriber::fmt::time::LocalTime;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initialize global tracing:
/// 1. Reads RUST_LOG or falls back to `default_level`  
/// 2. Formats with RFC-3339 timestamps to stdout
pub fn init(default_level: &str) {
    // build a layer that filters based on RUST_LOG (or our default)
    let filter_layer =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // build a formatting layer with local-time RFC3339 timestamps
    let fmt_layer = fmt::layer()
        .with_timer(LocalTime::rfc_3339())
        .with_writer(std::io::stdout);

    // compose and install the global subscriber
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
