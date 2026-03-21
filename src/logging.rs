use tracing_subscriber::EnvFilter;

pub fn init_logging(log_level: &str) -> Result<(), anyhow::Error> {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    Ok(())
}
