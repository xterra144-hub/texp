use serde::Deserialize;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("texp").join(".texp"))
        .unwrap_or_else(|| PathBuf::from(".texp"))
}
fn default_bookmarks() -> PathBuf {
    data_dir().join("bookmarks.txt")
}
fn default_skip_files() -> Vec<String> {
    vec!["Cargo.lock".into()]
}
fn default_skip_dirs() -> Vec<String> {
    vec![
        "C:\\Windows".into(),
        "C:\\$Recycle.Bin".into(),
        "C:\\System Volume Information".into(),
        "C:\\Documents and Settings".into(),
        "AppData".into(),
        "node_modules".into(),
        "target".into(),
        ".nuget".into(),
        ".git".into(),
        ".cargo".into(),
        "rustup".into(),
        "vcpkg".into(),
        "bin".into(),
        "release".into(),
        ".dotnet".into(),
        "net-10-windows".into(),
        "ProgramData\\Microsoft".into(),
        ".vscode".into(),
        ".cache".into(),
    ]
}

#[derive(Clone, Deserialize)]
pub struct General {
    #[serde(default = "default_bookmarks")]
    pub bookmarks_file: PathBuf,
}

#[derive(Clone, Deserialize)]
pub struct Indexing {
    #[serde(default = "default_skip_dirs")]
    pub skip_dirs: Vec<String>,
    #[serde(default = "default_skip_files")]
    pub skip_files: Vec<String>,
}

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub indexing: Indexing,
}

impl Config {
    pub fn load() -> Self {
        let _ = std::fs::create_dir_all(data_dir());
        let candidates = [
            dirs::config_dir().map(|d| d.join("texp").join("config.toml")),
            Some(PathBuf::from("texp.toml")),
        ];
        for path in candidates.iter().flatten() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            indexing: Indexing::default(),
        }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            bookmarks_file: default_bookmarks(),
        }
    }
}
impl Default for Indexing {
    fn default() -> Self {
        Self {
            skip_dirs: default_skip_dirs(),
            skip_files: default_skip_files(),
        }
    }
}
