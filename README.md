# mcp_server

A modular [Model Context Protocol](https://modelcontextprotocol.io/) server written in Rust.

Supports two transports (selected at runtime via `.env`):

| Transport | When to use |
|---|---|
| **stdio** | Claude Desktop, CLI clients, default |
| **Streamable HTTP** | Web clients, multi-session, remote access |

---

## Contents

- [Architecture](#architecture)
  - [How auto-discovery works](#how-auto-discovery-works)
- [Configuration (`.env`)](#configuration-env)
- [Transports](#transports)
  - [stdio](#stdio)
  - [Streamable HTTP](#streamable-http)
- [Testing](#testing)
  - [stdio](#stdio-1)
  - [Streamable HTTP](#streamable-http-1)
- [Build](#build)
- [Adding a new Tool](#adding-a-new-tool)
  - [Writing tests](#writing-tests)
- [Adding a new Resource](#adding-a-new-resource)
- [Adding a new Prompt](#adding-a-new-prompt)
- [Running tests](#running-tests)

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

## Testing 

The examples below cover both transports and exercise tools, resources, and
prompts. Adapt the tool name, resource URI, and prompt name to whatever you
have registered.

---

### stdio

The binary reads one JSON-RPC message per line from `stdin` and writes
responses to `stdout`. Pipe a newline-delimited sequence of messages directly
to the binary using a here-document.

#### Initialize

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
EOF
```

#### Confirm initialisation (notification — no response)

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
EOF
```

#### Tools — list

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
EOF
```

#### Tools — call (`count_lines`)

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"count_lines","arguments":{"path":"/tmp","extension":"rs"}}}
EOF
```

#### Resources — list

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"resources/list"}
EOF
```

#### Resources — read

Replace the URI with one returned by `resources/list`.

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"file:///config/settings.json"}}
EOF
```

#### Prompts — list

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"prompts/list"}
EOF
```

#### Prompts — get

Replace the name and arguments with values returned by `prompts/list`.

```bash
./target/release/mcp_server << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"summarise","arguments":{"text":"Hello world"}}}
EOF
```

Each response is written to `stdout` on its own line as it is processed.
Diagnostic/log output goes to `stderr` and will not interfere.

---

### Streamable HTTP

Start the server first:

```bash
MCP_COMMUNICATION=Streamable_HTTP ./target/release/mcp_server
```

Every request is a `POST /mcp`. The server returns a `MCP-Session-Id` header
in the `initialize` response; all subsequent requests must include that header.

#### 1. Initialize — capture the session ID

```bash
curl -si -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"curl-test","version":"0.0.1"}}}' \
  -D /tmp/mcp_headers.txt

# Extract the session ID from the saved headers
SESSION_ID=$(grep -i 'mcp-session-id' /tmp/mcp_headers.txt | awk '{print $2}' | tr -d '\r\n')
echo "Session: $SESSION_ID"
```

#### 2. Confirm initialisation (notification — no response body)

```bash
curl -s -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'
```

#### 3. Tools — list

```bash
curl -s -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
```

#### 4. Tools — call (`count_lines`)

```bash
curl -s -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"count_lines","arguments":{"path":"/tmp","extension":"rs"}}}'
```

#### 5. Resources — list

```bash
curl -s -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","id":4,"method":"resources/list"}'
```

#### 6. Resources — read

Replace the URI with one returned by `resources/list`.

```bash
curl -s -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":"file:///config/settings.json"}}'
```

#### 7. Prompts — list

```bash
curl -s -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","id":6,"method":"prompts/list"}'
```

#### 8. Prompts — get

Replace the name and arguments with values returned by `prompts/list`.

```bash
curl -s -X POST http://127.0.0.1:3333/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID" \
  -d '{"jsonrpc":"2.0","id":7,"method":"prompts/get","params":{"name":"summarise","arguments":{"text":"Hello world"}}}'
```

#### 9. End the session (optional)

```bash
curl -s -X DELETE http://127.0.0.1:3333/mcp \
  -H 'MCP-Protocol-Version: 2025-11-25' \
  -H "MCP-Session-Id: $SESSION_ID"
```

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
