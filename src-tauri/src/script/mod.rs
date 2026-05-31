use std::path::Path;

mod data;
mod engine;

pub use data::{collect_body_str, run_request_hooks, run_response_hooks, RequestData, ResponseData};

/// Load all `.js` files from a directory, sorted by filename.
pub fn load_scripts(dir: &Path) -> Vec<String> {
    let mut scripts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "js"))
            .collect();
        paths.sort();
        for path in paths {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if !content.trim().is_empty() {
                    scripts.push(content);
                }
            }
        }
    }
    scripts
}
