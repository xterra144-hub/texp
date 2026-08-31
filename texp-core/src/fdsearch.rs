use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static FD_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// проверка установлен ли fd в path 
pub fn available() -> bool {
    *FD_AVAILABLE.get_or_init(|| {
        Command::new("fd")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}


/// Search file names under `root` with fd. Returns `None` when fd is not
/// installed so the caller can fall back to the built-in index.
pub fn search(root: &Path, query: &str, show_hidden: bool, skip_dirs: &[String], limit: usize) -> Option<Vec<PathBuf>> {
    if !available() {
        return None;
    }
    if query.is_empty() {
        return Some(Vec::new());
    }
    let mut cmd = Command::new("fd");
    cmd.arg("--absolute-path")
        .arg(format!("--max-results={}", limit.max(1)))
        .arg("--max-depth=8");
    if show_hidden {
        cmd.arg("--hidden");
    }
    for dir in skip_dirs {
        if !dir.is_empty() {
            cmd.arg("--exclude").arg(dir);
        }
    }
    cmd.arg(query).arg(root);

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return Some(Vec::new());
    }
    let paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .collect();
    Some(paths)
}
