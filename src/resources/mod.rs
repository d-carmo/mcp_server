use std::future::Future;
use std::pin::Pin;

use anyhow::Result;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Boxed, object-safe async future used as the return type of [`McpResource::read`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Contract that every resource must implement.
///
/// # How to add a new resource
///
/// 1. Create `src/resources/<your_resource>.rs`.
/// 2. Define a unit struct and implement this trait on it.
/// 3. Register it with one line at module level:
///    ```rust,ignore
///    inventory::submit! { ResourceRegistration { factory: || Box::new(YourResource) } }
///    ```
/// 4. Run `cargo build` — the build script detects the new file and the
///    resource is live.  No other files need to be touched.
pub trait McpResource: Send + Sync + 'static {
    /// Unique URI that identifies this resource (e.g. `"file:///data/config.json"`).
    fn uri(&self) -> &'static str;

    /// Display name shown in resource listings.
    fn name(&self) -> &'static str;

    /// Optional description surfaced in resource listings.
    fn description(&self) -> Option<&'static str>;

    /// MIME type of the content returned by [`read`](McpResource::read)
    /// (e.g. `"text/plain"`, `"application/json"`).
    fn mime_type(&self) -> Option<&'static str>;

    /// Fetch and return the resource content as a UTF-8 string.
    fn read(&self) -> BoxFuture<'_, Result<String>>;
}

// ---------------------------------------------------------------------------
// Inventory registration
// ---------------------------------------------------------------------------

/// One registration entry per resource file.  Submit via [`inventory::submit!`].
pub struct ResourceRegistration {
    pub factory: fn() -> Box<dyn McpResource>,
}

inventory::collect!(ResourceRegistration);

// ---------------------------------------------------------------------------
// Auto-generated module declarations
//
// build.rs scans src/resources/ and writes one `pub mod <name>;` line per
// file into OUT_DIR/resources_modules.rs.
// ---------------------------------------------------------------------------

include!(concat!(env!("OUT_DIR"), "/resources_modules.rs"));

// ---------------------------------------------------------------------------
// Collector
// ---------------------------------------------------------------------------

/// Returns a fresh instance of every registered resource.
pub fn all_resources() -> Vec<Box<dyn McpResource>> {
    inventory::iter::<ResourceRegistration>()
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
    fn all_resources_have_valid_uris_and_names() {
        for resource in all_resources() {
            assert!(
                !resource.uri().is_empty(),
                "a resource has an empty URI"
            );
            assert!(
                !resource.name().is_empty(),
                "resource '{}' has an empty name",
                resource.uri()
            );
        }
    }
}
