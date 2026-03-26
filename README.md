# mcp_server

A modular [Model Context Protocol](https://modelcontextprotocol.io/) server written in Rust.

Supports two transports (selected at runtime via `.env`):

| Transport | When to use |
|---|---|
| **stdio** | Claude Desktop, CLI clients, default |
| **Streamable HTTP** | Web clients, multi-session, remote access |

---

## Architecture

```
src/
├── main.rs               # Wires config → engine.  Nothing else lives here.
├── config.rs             # Reads .env / env vars → Config struct
├── server.rs             # McpServer — implements rmcp::ServerHandler using
│                         # all registered tools, resources, and prompts
├── engine/
│   ├── mod.rs            # Dispatches to stdio or HTTP based on Config
│   └── http.rs           # Streamable HTTP transport (Axum + rmcp)
├── tools/
│   ├── mod.rs            # McpTool trait + ToolRegistration + all_tools()
│   └── count_lines.rs    # Built-in example tool
├── resources/
│   └── mod.rs            # McpResource trait + ResourceRegistration + all_resources()
└── prompts/
    └── mod.rs            # McpPrompt trait + PromptRegistration + all_prompts()
```

### How auto-discovery works

Three layers work together so that dropping a file into a folder is enough:

1. **`build.rs`** — runs before `rustc` on every `cargo build`.  It scans
   `src/tools/`, `src/resources/`, and `src/prompts/` and writes one
   `pub mod <name>;` line per `.rs` file into `OUT_DIR/<dir>_modules.rs`.

2. **`include!`** — each `mod.rs` has a single
   `include!(concat!(env!("OUT_DIR"), "/<dir>_modules.rs"))` that pulls in
   the generated declarations.  The Rust compiler sees every file in the
   folder without any manual work.

3. **`inventory` crate** — each plugin file calls `inventory::submit!` once
   at module level.  At link time the linker collects all submissions.
   `all_tools()` / `all_resources()` / `all_prompts()` iterate over them at
   startup.

---

## Configuration (`.env`)

Copy `.env` and edit as needed.  All values have built-in defaults.

| Variable | Default | Description |
|---|---|---|
| `MCP_PORT` | `3333` | TCP port for the Streamable HTTP transport |
| `MCP_LISTENING_ADDRESS` | `127.0.0.1` | Bind address for the Streamable HTTP transport |
| `MCP_COMMUNICATION` | `stdio` | Transport: `stdio` or `Streamable_HTTP` |

Example — run as an HTTP server on all interfaces:

```env
MCP_COMMUNICATION=Streamable_HTTP
MCP_LISTENING_ADDRESS=0.0.0.0
MCP_PORT=3333
```

---

## Transports

### stdio

The default.  The client spawns this binary as a subprocess.  JSON-RPC
messages are exchanged over stdin/stdout separated by newlines.
`eprintln!` (stderr) is used for all logging so as not to corrupt the
protocol channel.

MCP client configuration example (Claude Desktop / `.mcp.json`):

```json
{
  "mcpServers": {
    "mcp-server": {
      "command": "/path/to/target/release/mcp_server"
    }
  }
}
```

### Streamable HTTP

Implements the [Streamable HTTP transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
from the MCP spec (2025-11-25).

The server mounts a single endpoint at `/mcp`:

| Method | Purpose |
|---|---|
| `POST /mcp` | Send a JSON-RPC message.  Response is `application/json` or `text/event-stream` depending on message type. |
| `GET /mcp` | Open a persistent SSE stream for server-initiated messages. |
| `DELETE /mcp` | Explicitly close the session. |

Session management (`MCP-Session-Id`), `MCP-Protocol-Version` header
validation, SSE keep-alive, and stream resumption are all handled internally
by rmcp's `StreamableHttpService`.

---

## Build

```bash
cargo build --release
```

The build script runs automatically — no manual steps required.

---

## Adding a new Tool

1. **Create** `src/tools/<your_tool>.rs`.

2. **Implement** the `McpTool` trait:

```rust
use anyhow::Result;
use serde_json::{json, Map, Value};
use crate::tools::{BoxFuture, McpTool, ToolRegistration};

pub struct MyTool;

impl McpTool for MyTool {
    fn name(&self) -> &'static str {
        "my_tool"   // unique snake_case identifier
    }

    fn description(&self) -> &'static str {
        "Does something useful"
    }

    fn schema(&self) -> Map<String, Value> {
        // Must be a JSON Schema object with "type": "object"
        json!({
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "The input value" }
            },
            "required": ["input"]
        })
        .as_object()
        .unwrap()
        .clone()
    }

    fn call(&self, params: Map<String, Value>) -> BoxFuture<'_, Result<String>> {
        Box::pin(async move {
            let input = params["input"].as_str().unwrap_or("");
            Ok(format!("You said: {}", input))
        })
    }
}
```

3. **Register** it at the bottom of the same file:

```rust
inventory::submit! { ToolRegistration { factory: || Box::new(MyTool) } }
```

4. **Run `cargo build`** — the tool is live.  No other file needs editing.

### Writing tests

Add a `#[cfg(test)]` module in the same file and test `call` directly:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_expected_output() {
        let tool = MyTool;
        let params = serde_json::json!({ "input": "hello" });
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.call(params.as_object().unwrap().clone()))
            .unwrap();
        assert_eq!(result, "You said: hello");
    }
}
```

---

## Adding a new Resource

1. **Create** `src/resources/<your_resource>.rs`.

2. **Implement** the `McpResource` trait:

```rust
use anyhow::Result;
use crate::resources::{BoxFuture, McpResource, ResourceRegistration};

pub struct ConfigResource;

impl McpResource for ConfigResource {
    fn uri(&self) -> &'static str {
        "file:///config/settings.json"   // unique URI
    }

    fn name(&self) -> &'static str {
        "Application Settings"
    }

    fn description(&self) -> Option<&'static str> {
        Some("Current application configuration")
    }

    fn mime_type(&self) -> Option<&'static str> {
        Some("application/json")
    }

    fn read(&self) -> BoxFuture<'_, Result<String>> {
        Box::pin(async move {
            // Read from disk, database, network, etc.
            Ok(r#"{"theme": "dark"}"#.to_string())
        })
    }
}
```

3. **Register** it:

```rust
inventory::submit! { ResourceRegistration { factory: || Box::new(ConfigResource) } }
```

4. **Run `cargo build`**.

---

## Adding a new Prompt

1. **Create** `src/prompts/<your_prompt>.rs`.

2. **Implement** the `McpPrompt` trait:

```rust
use anyhow::Result;
use serde_json::{Map, Value};
use crate::prompts::{
    BoxFuture, McpPrompt, PromptArg, PromptMessage, PromptRegistration, Role,
};

pub struct SummarisePrompt;

impl McpPrompt for SummarisePrompt {
    fn name(&self) -> &'static str {
        "summarise"   // unique identifier
    }

    fn description(&self) -> Option<&'static str> {
        Some("Summarise a piece of text")
    }

    fn arguments(&self) -> Vec<PromptArg> {
        vec![PromptArg {
            name: "text",
            description: Some("The text to summarise"),
            required: true,
        }]
    }

    fn get(&self, args: Map<String, Value>) -> BoxFuture<'_, Result<Vec<PromptMessage>>> {
        Box::pin(async move {
            let text = args["text"].as_str().unwrap_or("(no text provided)");
            Ok(vec![PromptMessage {
                role: Role::User,
                text: format!("Please summarise the following:\n\n{}", text),
            }])
        })
    }
}
```

3. **Register** it:

```rust
inventory::submit! { PromptRegistration { factory: || Box::new(SummarisePrompt) } }
```

4. **Run `cargo build`**.

---

## Running tests

```bash
cargo test
```

Each module ships its own unit tests.  The tool tests in particular exercise
the actual call logic end-to-end using a real filesystem.
