use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Boxed, object-safe async future used as the return type of [`McpPrompt::get`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Role of a message in a prompt conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Role {
    User,
    Assistant,
}

/// A single message returned by [`McpPrompt::get`].
#[derive(Debug, Clone)]
pub struct PromptMessage {
    pub role: Role,
    pub text: String,
}

/// A single argument that a prompt accepts.
#[derive(Debug, Clone)]
pub struct PromptArg {
    /// Argument name as it will appear in the protocol.
    pub name: &'static str,
    /// Optional human-readable description.
    pub description: Option<&'static str>,
    /// Whether the argument must be provided by the caller.
    pub required: bool,
}

/// Contract that every prompt must implement.
///
/// # How to add a new prompt
///
/// 1. Create `src/prompts/<your_prompt>.rs`.
/// 2. Define a unit struct and implement this trait on it.
/// 3. Register it with one line at module level:
///    ```rust,ignore
///    inventory::submit! { PromptRegistration { factory: || Box::new(YourPrompt) } }
///    ```
/// 4. Run `cargo build` — the build script detects the new file and the
///    prompt is live.  No other files need to be touched.
pub trait McpPrompt: Send + Sync + 'static {
    /// Unique identifier (e.g. `"summarise_file"`).
    fn name(&self) -> &'static str;

    /// Optional description surfaced in prompt listings.
    fn description(&self) -> Option<&'static str>;

    /// Arguments the prompt accepts.
    fn arguments(&self) -> Vec<PromptArg>;

    /// Render the prompt with the provided arguments and return a conversation.
    fn get(&self, args: Map<String, Value>) -> BoxFuture<'_, Result<Vec<PromptMessage>>>;
}

// ---------------------------------------------------------------------------
// Inventory registration
// ---------------------------------------------------------------------------

/// One registration entry per prompt file.  Submit via [`inventory::submit!`].
pub struct PromptRegistration {
    pub factory: fn() -> Box<dyn McpPrompt>,
}

inventory::collect!(PromptRegistration);

// ---------------------------------------------------------------------------
// Auto-generated module declarations
//
// build.rs scans src/prompts/ and writes one `pub mod <name>;` line per file
// into OUT_DIR/prompts_modules.rs.
// ---------------------------------------------------------------------------

include!(concat!(env!("OUT_DIR"), "/prompts_modules.rs"));

// ---------------------------------------------------------------------------
// Collector
// ---------------------------------------------------------------------------

/// Returns a fresh instance of every registered prompt.
pub fn all_prompts() -> Vec<Box<dyn McpPrompt>> {
    inventory::iter::<PromptRegistration>()
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
    fn all_prompts_have_non_empty_names() {
        for prompt in all_prompts() {
            assert!(
                !prompt.name().is_empty(),
                "a prompt has an empty name"
            );
        }
    }
}
