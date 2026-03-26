use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Boxed, object-safe async future used as the return type of [`McpTool::call`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Contract that every tool must implement.
///
/// # How to add a new tool
///
/// 1. Create `src/tools/<your_tool>.rs`.
/// 2. Define a unit struct and implement this trait on it.
/// 3. Register it with one line at module level:
///    ```rust,ignore
///    inventory::submit! { ToolRegistration { factory: || Box::new(YourTool) } }
///    ```
/// 4. Run `cargo build` — the build script detects the new file and the tool
///    is live.  No other files need to be touched.
pub trait McpTool: Send + Sync + 'static {
    /// Unique snake_case identifier used in the MCP protocol (e.g. `"count_lines"`).
    fn name(&self) -> &'static str;

    /// Human-readable description surfaced in tool listings.
    fn description(&self) -> &'static str;

    /// JSON Schema object (`"type": "object"`) describing the tool's input.
    fn schema(&self) -> Map<String, Value>;

    /// Execute the tool.  Returns a plain text result or an error message.
    fn call(&self, params: Map<String, Value>) -> BoxFuture<'_, Result<String>>;
}

// ---------------------------------------------------------------------------
// Inventory registration
// ---------------------------------------------------------------------------

/// One registration entry per tool file.  Submit via [`inventory::submit!`].
pub struct ToolRegistration {
    pub factory: fn() -> Box<dyn McpTool>,
}

inventory::collect!(ToolRegistration);

// ---------------------------------------------------------------------------
// Auto-generated module declarations
//
// build.rs scans src/tools/ and writes one `pub mod <name>;` line per file
// into OUT_DIR/tools_modules.rs.  Adding a file to the directory is enough —
// no manual edit here is required.
// ---------------------------------------------------------------------------

include!(concat!(env!("OUT_DIR"), "/tools_modules.rs"));

// ---------------------------------------------------------------------------
// Collector
// ---------------------------------------------------------------------------

/// Returns a fresh instance of every registered tool.
pub fn all_tools() -> Vec<Box<dyn McpTool>> {
    inventory::iter::<ToolRegistration>()
        .map(|r| (r.factory)())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_least_one_tool_is_registered() {
        assert!(
            !all_tools().is_empty(),
            "no tools registered — check inventory::submit! calls in src/tools/"
        );
    }

    #[test]
    fn all_tools_have_non_empty_metadata() {
        for tool in all_tools() {
            assert!(!tool.name().is_empty(), "a tool has an empty name");
            assert!(
                !tool.description().is_empty(),
                "tool '{}' has an empty description",
                tool.name()
            );
        }
    }

    #[test]
    fn tool_schemas_are_objects() {
        for tool in all_tools() {
            let schema = tool.schema();
            assert_eq!(
                schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "tool '{}' schema must have \"type\": \"object\"",
                tool.name()
            );
        }
    }
}
