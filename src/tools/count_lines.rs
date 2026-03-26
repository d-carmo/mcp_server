use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{json, Map, Value};

use super::{BoxFuture, McpTool, ToolRegistration};

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

pub struct CountLinesTool;

impl McpTool for CountLinesTool {
    fn name(&self) -> &'static str {
        "count_lines"
    }

    fn description(&self) -> &'static str {
        "Count total lines of files matching a given extension within a given directory"
    }

    fn schema(&self) -> Map<String, Value> {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to search in"
                },
                "extension": {
                    "type": "string",
                    "description": "File extension to filter by (e.g. rs, txt, js). Do not include the dot."
                }
            },
            "required": ["path", "extension"]
        })
        .as_object()
        .expect("schema literal is a valid JSON object")
        .clone()
    }

    fn call(&self, params: Map<String, Value>) -> BoxFuture<'_, Result<String>> {
        Box::pin(async move {
            let path_str = params
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let extension = params
                .get("extension")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            let path = PathBuf::from(&path_str);

            if !path.exists() {
                return Ok(format!("Error: path '{}' does not exist", path_str));
            }
            if !path.is_dir() {
                return Ok(format!("Error: path '{}' is not a directory", path_str));
            }

            let mut total_lines: u64 = 0;
            let mut file_count: u64 = 0;

            match count_lines_recursive(&path, &extension, &mut total_lines, &mut file_count) {
                Ok(()) => Ok(format!(
                    "Found {} .{} files containing {} total lines in {}",
                    file_count, extension, total_lines, path_str
                )),
                Err(e) => Ok(format!("Error counting lines: {}", e)),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

fn count_lines_recursive(
    dir: &Path,
    extension: &str,
    total_lines: &mut u64,
    file_count: &mut u64,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.')
                || matches!(name.as_ref(), "target" | "node_modules" | "vendor")
            {
                continue;
            }
            count_lines_recursive(&path, extension, total_lines, file_count)?;
        } else if path.extension().map_or(false, |e| e == extension) {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    *total_lines += content.lines().count() as u64;
                    *file_count += 1;
                }
                Err(_) => eprintln!("Warning: could not read {}", path.display()),
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Inventory registration — picked up automatically at link time
// ---------------------------------------------------------------------------

inventory::submit! { ToolRegistration { factory: || Box::new(CountLinesTool) } }

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn setup_tree(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mcp_count_lines_{}", suffix));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, content: &str) {
        write!(File::create(path).unwrap(), "{}", content).unwrap();
    }

    #[test]
    fn counts_matching_extension_only() {
        let dir = setup_tree("basic");
        write(&dir.join("a.rs"), "line1\nline2\nline3");
        write(&dir.join("b.rs"), "x\ny");
        write(&dir.join("c.txt"), "ignored");

        let mut total = 0u64;
        let mut count = 0u64;
        count_lines_recursive(&dir, "rs", &mut total, &mut count).unwrap();

        assert_eq!(count, 2);
        assert_eq!(total, 5);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn skips_hidden_and_vendor_dirs() {
        let dir = setup_tree("skip_dirs");

        let hidden = dir.join(".git");
        fs::create_dir_all(&hidden).unwrap();
        write(&hidden.join("config.rs"), "should\nnot\ncount");

        let vendor = dir.join("vendor");
        fs::create_dir_all(&vendor).unwrap();
        write(&vendor.join("lib.rs"), "also skipped");

        write(&dir.join("main.rs"), "counted");

        let mut total = 0u64;
        let mut count = 0u64;
        count_lines_recursive(&dir, "rs", &mut total, &mut count).unwrap();

        assert_eq!(count, 1);
        assert_eq!(total, 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn returns_error_string_for_missing_path() {
        let tool = CountLinesTool;
        let params = serde_json::json!({
            "path": "/this/path/does/not/exist/ever",
            "extension": "rs"
        });

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.call(params.as_object().unwrap().clone()))
            .unwrap();

        assert!(result.starts_with("Error:"), "got: {}", result);
    }
}
