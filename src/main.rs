use {
    tokio::main,
    tracing_subscriber::{EnvFilter, fmt},
};

#[main]
async fn main() {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();
}
