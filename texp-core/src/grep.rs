use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub line: usize,
    pub word: usize,
    pub text: String,
    pub file: PathBuf,
}
pub struct Searcher;
pub struct RegexHint {
    pub is_valid: bool,
    pub message: String,
}
impl Searcher {
    pub fn checker_pattern(raw_pattern: &str) -> RegexHint {
        if !raw_pattern.starts_with("re:") {
            return RegexHint {
                is_valid: true,
                message: "Plain text search (case-insensitive)".to_string(),
            };
        }
        let pattern = raw_pattern.strip_prefix("re:").unwrap_or("");
        if pattern.is_empty() {
            return RegexHint {
                is_valid: true,
                message: "Enter regex pattern after 're:'".to_string(),
            };
        }
        match Regex::new(pattern) {
            Ok(re) => {
                let group_count = re.captures_len() - 1;
                let msg = if group_count > 0 {
                    format!("Valid syntax. Capture groups: {}", group_count)
                } else {
                    "Valid regex syntax".to_string()
                };
                RegexHint {
                    is_valid: true,
                    message: msg,
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                let short_error = error_msg
                    .lines()
                    .next()
                    .unwrap_or("Syntax error")
                    .to_string();
                RegexHint {
                    is_valid: false,
                    message: format!("Error: {}", short_error),
                }
            }
        }
    }
    pub fn search_with_gitignore(current_dir: &Path,pattern: &str,skip_dirs: &[String],skip_files: &[String],) -> Vec<Match> {
        let mut matches = Vec::new();
        let (is_regex, body) = if let Some(p) = pattern.strip_prefix("re:") {
            (true, p.to_string())
        } else {
            (false, regex::escape(pattern))
        };
        let regex_str = if is_regex {
            body
        } else {
            format!("(?i){}", body)
        };
        let regex = match Regex::new(&regex_str) {
            Ok(regex) => regex,
            Err(_) => return matches,
        };
        use ignore::WalkBuilder;
        let dir_skip = skip_dirs
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>();
        let file_skip = skip_files
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>();
        let walker = WalkBuilder::new(current_dir)
            .hidden(true)
            .git_ignore(true)
            .filter_entry(move |entry| {
                let path = entry.path();
                if path.is_dir() {
                    let lower = path.to_string_lossy().to_lowercase();
                    !dir_skip.iter().any(|s| lower.contains(s.as_str()))
                } else {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    !file_skip.iter().any(|s| s == &name.to_lowercase())
                }
            })
            .build();
        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    for (line_index, line) in content.lines().enumerate() {
                        if let Some(mat) = regex.find(line) {
                            matches.push(Match {
                                file: path.to_path_buf(),
                                line: line_index + 1,
                                word: mat.start() + 1,
                                text: line.to_string(),
                            })
                        };
                    }
                }
            }
        }
        matches
    }
}