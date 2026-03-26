use anyhow::Result;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::local::LocalSessionManager,
};
use tokio::net::TcpListener;

use crate::server::McpServer;

/// Start the Streamable HTTP transport.
///
/// The MCP endpoint is mounted at `/mcp` and handles all three methods
/// required by the spec (POST, GET, DELETE).
///
/// - `POST /mcp`   — send a JSON-RPC message; response is JSON or SSE
/// - `GET  /mcp`   — open a persistent SSE stream for server-initiated messages
/// - `DELETE /mcp` — close the session explicitly
///
/// Session management, `MCP-Session-Id` headers, `MCP-Protocol-Version`
/// validation, and SSE keep-alive are all handled internally by rmcp's
/// [`StreamableHttpService`].
///
/// Binds to `addr` (e.g. `"127.0.0.1:3333"`).
pub async fn serve(addr: &str) -> Result<()> {
    let config = StreamableHttpServerConfig {
        // Keep SSE connections alive with periodic pings.
        sse_keep_alive: Some(std::time::Duration::from_secs(15)),
        // Tell clients how long to wait before reconnecting after a dropped stream.
        sse_retry: Some(std::time::Duration::from_secs(3)),
        // Stateful mode: sessions are maintained server-side (required for SSE).
        stateful_mode: true,
        ..Default::default()
    };

    // A new McpServer instance is created per session.  Tools, resources, and
    // prompts are all stateless so this is both correct and cheap.
    let service: StreamableHttpService<McpServer, LocalSessionManager> =
        StreamableHttpService::new(|| Ok(McpServer::new()), Default::default(), config);

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Resolves when SIGINT (Ctrl-C) or SIGTERM is received.
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    {
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler")
                .recv()
                .await;
        };
        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    #[cfg(not(unix))]
    ctrl_c.await;

    eprintln!("[mcp_server] shutting down");
}
