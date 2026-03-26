pub mod http;

use anyhow::Result;
use rmcp::ServiceExt;
use tracing::info;

use crate::config::{Config, TransportMode};
use crate::server::McpServer;

/// Owns the selected transport and drives the server loop.
pub struct Engine {
    config: Config,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Start the server on whichever transport `MCP_COMMUNICATION` selects.
    pub async fn run(self) -> Result<()> {
        match self.config.transport {
            TransportMode::Stdio => {
                info!("transport: stdio");
                Self::run_stdio().await
            }
            TransportMode::StreamableHttp => {
                let addr = format!("{}:{}", self.config.address, self.config.port);
                info!("transport: Streamable HTTP  →  http://{}/mcp", addr);
                http::serve(&addr).await
            }
        }
    }

    async fn run_stdio() -> Result<()> {
        let server = McpServer::new();
        let transport = rmcp::transport::io::stdio();
        let handle = server.serve(transport).await?;
        handle.waiting().await?;
        Ok(())
    }
}
