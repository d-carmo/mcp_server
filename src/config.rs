use anyhow::{Context, Result};

/// Which transport the server will listen on.
#[derive(Debug, Clone, PartialEq)]
pub enum TransportMode {
    /// JSON-RPC over stdin/stdout (default).
    Stdio,
    /// Streamable HTTP as defined in the MCP spec (2025-11-25).
    StreamableHttp,
}

/// Runtime configuration loaded from `.env` and environment variables.
///
/// Variable precedence: environment variable > `.env` file > built-in default.
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP port for the HTTP transport.  Default: `3333`.
    pub port: u16,
    /// Bind address for the HTTP transport.  Default: `127.0.0.1`.
    pub address: String,
    /// Active transport mode.  Default: [`TransportMode::Stdio`].
    pub transport: TransportMode,
}

impl Config {
    /// Load configuration from `.env` (if present) and the process environment.
    pub fn from_env() -> Result<Self> {
        // Ignore a missing .env — it is optional.
        dotenvy::dotenv().ok();

        let port = std::env::var("MCP_PORT")
            .unwrap_or_else(|_| "3333".to_string())
            .parse::<u16>()
            .context("MCP_PORT must be a valid port number (1–65535)")?;

        let address = std::env::var("MCP_LISTENING_ADDRESS")
            .unwrap_or_else(|_| "127.0.0.1".to_string());

        let transport = match std::env::var("MCP_COMMUNICATION")
            .unwrap_or_else(|_| "stdio".to_string())
            .as_str()
        {
            "stdio" => TransportMode::Stdio,
            "Streamable_HTTP" => TransportMode::StreamableHttp,
            other => anyhow::bail!(
                "Unknown MCP_COMMUNICATION value: '{}'. \
                 Valid values are 'stdio' and 'Streamable_HTTP'.",
                other
            ),
        };

        Ok(Config {
            port,
            address,
            transport,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialise all config tests — they mutate the global environment.
    static LOCK: Mutex<()> = Mutex::new(());

    fn clear_vars() {
        // Safety: single-threaded test context.
        unsafe {
            std::env::remove_var("MCP_PORT");
            std::env::remove_var("MCP_LISTENING_ADDRESS");
            std::env::remove_var("MCP_COMMUNICATION");
        }
    }

    #[test]
    fn defaults_when_vars_absent() {
        let _g = LOCK.lock().unwrap();
        clear_vars();
        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.port, 3333);
        assert_eq!(cfg.address, "127.0.0.1");
        assert_eq!(cfg.transport, TransportMode::Stdio);
    }

    #[test]
    fn parses_http_transport() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("MCP_COMMUNICATION", "Streamable_HTTP");
            std::env::set_var("MCP_PORT", "8080");
            std::env::set_var("MCP_LISTENING_ADDRESS", "0.0.0.0");
        }
        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.transport, TransportMode::StreamableHttp);
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.address, "0.0.0.0");
        clear_vars();
    }

    #[test]
    fn rejects_invalid_transport() {
        let _g = LOCK.lock().unwrap();
        clear_vars();
        unsafe { std::env::set_var("MCP_COMMUNICATION", "websocket") };
        assert!(Config::from_env().is_err());
        unsafe { std::env::remove_var("MCP_COMMUNICATION") };
    }

    #[test]
    fn rejects_invalid_port() {
        let _g = LOCK.lock().unwrap();
        clear_vars();
        unsafe { std::env::set_var("MCP_PORT", "not_a_number") };
        assert!(Config::from_env().is_err());
        unsafe { std::env::remove_var("MCP_PORT") };
    }
}
