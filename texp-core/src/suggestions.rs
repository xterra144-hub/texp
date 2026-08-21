use std::path::Path;

use crate::fdsearch;

pub struct Suggestion {
    pub text: String,
    pub description: String,
}

pub struct SuggestionContext<'a> {
    pub input: &'a str,
    pub current_word: &'a str,
    pub cwd: &'a Path,
}

pub trait SuggestionProvider {
    fn suggestions(&mut self, ctx: &SuggestionContext) -> Vec<Suggestion>;
}

const COMMAND_SNIPPETS: &[(&str, &str)] = &[
    (":cd ", "Change directory"),
    (":cp ", "Copy selected file(s)"),
    (":mv ", "Move selected file(s)"),
    (":rn ", "Rename selected file"),
    (":rm", "Delete selected to Recycle Bin"),
    (":mkdir ", "Create directory"),
    (":touch ", "Create empty file"),
    (":find ", "Search files by name"),
    (":grep ", "Search file content"),
    (":du", "Disk usage analyzer"),
    (":index", "Rebuild file index"),
    (":q", "Quit"),
];

/// Built-in provider: suggests base commands while the user types
/// the first token of a command line.
pub struct CommandSnippetProvider;

impl SuggestionProvider for CommandSnippetProvider {
    fn suggestions(&mut self, ctx: &SuggestionContext) -> Vec<Suggestion> {
        // Only suggest while the user is still typing the command token.
        if ctx.input.contains(char::is_whitespace) {
            return Vec::new();
        }
        let prefix = ctx.current_word.to_lowercase();
        COMMAND_SNIPPETS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(&prefix))
            .map(|(cmd, desc)| Suggestion {
                text: cmd.to_string(),
                description: desc.to_string(),
            })
            .collect()
    }
}

const FD_MIN_QUERY_LEN: usize = 2;
const FD_SUGGESTION_LIMIT: usize = 30;

/// Provider: live path suggestions for `:find` backed by `fd`.
/// Silently returns nothing when fd is not installed (the command
/// itself falls back to the built-in index).
pub struct FdPathProvider;

impl SuggestionProvider for FdPathProvider {
    fn suggestions(&mut self, ctx: &SuggestionContext) -> Vec<Suggestion> {
        let query = match ctx.input.strip_prefix(":find ") {
            Some(rest) => rest,
            None => return Vec::new(),
        };
        if query.len() < FD_MIN_QUERY_LEN {
            return Vec::new();
        }
        let paths = fdsearch::search(ctx.cwd, query, true, &[], FD_SUGGESTION_LIMIT);
        let Some(paths) = paths else {
            return Vec::new();
        };
        paths
            .into_iter()
            .map(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.to_string_lossy().to_string());
                let text = if p.is_dir() {
                    format!("{}/", name)
                } else {
                    name
                };
                Suggestion { text, description: String::new() }
            })
            .collect()
    }
}
