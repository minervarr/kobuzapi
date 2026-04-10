//! Binary entrypoint for the Qobuz API client.

use {
    anyhow::{Result, anyhow},
    tokio::main,
    tracing_subscriber::{EnvFilter, fmt},
};

/// Application entrypoint.
///
/// # Errors
///
/// Returns an error if the tracing subscriber fails to initialize.
///
/// # Panics
///
/// Panics if the Tokio runtime fails to initialize.
#[main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|e| anyhow!(e))?;

    Ok(())
}
