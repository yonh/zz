use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::Layer;
use tracing_subscriber::prelude::*;

pub fn init_logging(
    log_level: &str,
    trace_layer: Option<crate::trace_layer::JournalTraceLayer>,
) -> Result<(), anyhow::Error> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))?;

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr);

    if let Some(layer) = trace_layer {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
    }

    Ok(())
}
