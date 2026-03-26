# Engine

The engine is the entry point of the MCP server. It is responsible for loading runtime
configuration and driving the server loop over whichever transport is selected.

## Files

| File | Responsibility |
|------|----------------|
| [mod.rs](mod.rs) | `Engine` struct — reads config, dispatches to the correct transport |
| [http.rs](http.rs) | Streamable HTTP transport implementation (Axum + rmcp) |

## Startup sequence

```
main()
  └─ Config::from_env()          # load .env + env vars → Config
  └─ Engine::new(config)
  └─ Engine::run()
        ├─ TransportMode::Stdio          → run_stdio()
        └─ TransportMode::StreamableHttp → http::serve(addr)
```

`main` logs to **stderr** exclusively. `stdout` is reserved for the MCP protocol
wire format and must never receive plain text output (no `println!`).

## Transport modes

Transport is selected via the `MCP_COMMUNICATION` environment variable (or `.env`).

### stdio (default)

```
MCP_COMMUNICATION=stdio
```

A single `McpServer` instance is created and wired to `rmcp::transport::io::stdio()`.
The process reads JSON-RPC messages from `stdin` and writes responses to `stdout`.
The server loop runs until the peer closes the stream.

```
client
  │  JSON-RPC over stdin/stdout
  ▼
McpServer (rmcp ServerHandler)
```

### Streamable HTTP

```
MCP_COMMUNICATION=Streamable_HTTP
MCP_LISTENING_ADDRESS=127.0.0.1   # default
MCP_PORT=3333                     # default
```

An Axum router binds to `<address>:<port>` and mounts rmcp's
`StreamableHttpService` at `/mcp`. The service handles the three HTTP methods
required by the MCP spec:

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/mcp` | Send a JSON-RPC request; response is JSON or SSE |
| `GET`  | `/mcp` | Open a persistent SSE stream for server-initiated messages |
| `DELETE` | `/mcp` | Close the session explicitly |

Session management, `MCP-Session-Id` headers, `MCP-Protocol-Version` validation,
and SSE keep-alive pings are handled internally by rmcp's `StreamableHttpService`.
A fresh `McpServer` instance is created per session (stateful mode).

SSE is configured with:
- **keep-alive ping**: every 15 seconds
- **retry hint** sent to clients: 3 seconds

The server shuts down gracefully on `SIGINT` (Ctrl-C) or `SIGTERM`.

## McpServer

`McpServer` (defined in [`../server.rs`](../server.rs)) implements rmcp's
`ServerHandler` trait. On construction it collects all registered tools,
resources, and prompts via the `inventory` crate and holds them as
`Vec<Box<dyn Mc*>>` trait objects. It is stateless with respect to individual
requests, making it safe and cheap to instantiate per-session.

```
McpServer
  ├─ tools:     Vec<Box<dyn McpTool>>
  ├─ resources: Vec<Box<dyn McpResource>>
  └─ prompts:   Vec<Box<dyn McpPrompt>>
```

Handler methods implemented:

| Method | MCP operation |
|--------|---------------|
| `get_info` | `initialize` — advertises capabilities and server version |
| `list_tools` / `call_tool` | `tools/list`, `tools/call` |
| `list_resources` / `read_resource` | `resources/list`, `resources/read` |
| `list_prompts` / `get_prompt` | `prompts/list`, `prompts/get` |

## Dynamic plugin loading

Tools, resources, and prompts are discovered at **compile time** using two
mechanisms that together require no manual registration outside of the plugin
file itself:

1. **`build.rs` module scanning** — the build script scans `src/tools/`,
   `src/resources/`, and `src/prompts/` and writes a `pub mod <name>;`
   declaration for every `.rs` file found (excluding `mod.rs`). The generated
   file is `include!`-d into each `mod.rs`, so new files are compiled
   automatically.

2. **`inventory` crate** — each plugin file contains a single call to
   `inventory::submit!` at module level, registering a factory function.
   At link time the linker stitches all submitted entries into a global
   iterable collection. `all_tools()` / `all_resources()` / `all_prompts()`
   iterate that collection to produce the `Vec` used by `McpServer`.

Adding a new tool means:
1. Create `src/tools/<name>.rs`.
2. Implement `McpTool` on a unit struct.
3. Add `inventory::submit! { ToolRegistration { factory: || Box::new(YourTool) } }`.
4. Run `cargo build`. No other file needs to change.

The same pattern applies to resources (`McpResource`) and prompts (`McpPrompt`).
