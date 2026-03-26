use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Scans each plugin directory and emits a `<dir>_modules.rs` file into OUT_DIR.
///
/// Each entry is `#[path = "<absolute>"] pub mod <name>;` so that the
/// `include!` in the respective mod.rs resolves to the correct source file
/// regardless of where OUT_DIR lives.
fn main() {
    generate_module_list("src/tools");
    generate_module_list("src/resources");
    generate_module_list("src/prompts");
}

fn generate_module_list(dir: &str) {
    // Re-run if anything in the directory changes.
    println!("cargo:rerun-if-changed={}", dir);

    let module_name = Path::new(dir)
        .file_name()
        .expect("dir has a file name")
        .to_string_lossy();

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by Cargo");
    let dest = format!("{}/{}_modules.rs", out_dir, module_name);

    // Canonicalize to get a stable absolute path.
    let abs_dir = fs::canonicalize(dir).unwrap_or_else(|_| PathBuf::from(dir));

    let mut modules: Vec<(String, String)> = Vec::new();

    if abs_dir.exists() {
        for entry in fs::read_dir(&abs_dir).expect("can read plugin dir") {
            let entry = entry.expect("valid dir entry");
            let path = entry
                .path()
                .canonicalize()
                .unwrap_or_else(|_| entry.path());
            let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

            if file_name.ends_with(".rs") && file_name != "mod.rs" {
                let mod_name = file_name.trim_end_matches(".rs").to_string();
                // Use forward slashes — Rust accepts them on all platforms.
                let abs_path = path.to_string_lossy().replace('\\', "/");
                modules.push((mod_name, abs_path));
            }
        }
    }

    modules.sort();

    // Emit `#[path = "..."] pub mod <name>;` so the declaration resolves to
    // the actual source file, not a path relative to OUT_DIR.
    let content = modules
        .iter()
        .map(|(name, path)| format!("#[path = \"{}\"]\npub mod {};", path, name))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&dest, content).expect("can write generated module list");
}
