mod config;
mod engine;
mod prompts;
mod resources;
mod server;
mod tools;

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // All logging goes to stderr — stdout is reserved for the MCP protocol.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let config = config::Config::from_env()?;
    info!(
        transport = ?config.transport,
        address   = %config.address,
        port      = config.port,
        "mcp_server starting"
    );

    engine::Engine::new(config).run().await
}
