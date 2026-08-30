use crate::grep::Match;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};
#[derive(Clone)]
pub struct DirEntry {
    pub is_dir: bool,
    pub modified: Option<SystemTime>,
    pub size: u64,
}

#[derive(Clone, Copy)]
pub enum SortMode {
    ByName,
    ByDate,
    BySize,
    ByType,
}

#[derive(Clone)]
pub struct DiskItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

pub struct NavState {
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub cursor_index: usize,
    pub selected_files: HashSet<PathBuf>,
    pub path_segments: Vec<PathBuf>,
    pub path_cursor: usize,
    pub sort_mode: SortMode,
    pub sort_reverse: bool,
    pub filter_input: String,
    pub full_files: Vec<PathBuf>,
    pub dir_cache: HashMap<PathBuf, DirEntry>,
    pub dir_child_count: HashMap<PathBuf, usize>,
    pub history: Vec<PathBuf>,
    pub history_pos: usize,
    pub last_dir_refresh: HashMap<PathBuf, Instant>,
    pub show_hidden: bool,
    pub filter_active: bool,
}

impl NavState {
    pub fn new(current_dir: PathBuf) -> Self {
        Self {
            current_dir,
            files: Vec::new(),
            cursor_index: 0,
            selected_files: HashSet::new(),
            path_segments: Vec::new(),
            path_cursor: 0,
            sort_mode: SortMode::ByName,
            sort_reverse: false,
            filter_input: String::new(),
            full_files: Vec::new(),
            dir_cache: HashMap::new(),
            dir_child_count: HashMap::new(),
            history: vec![],
            history_pos: 0,
            last_dir_refresh: HashMap::new(),
            show_hidden: false,
            filter_active: false,
        }
    }
}
pub struct PreviewState {
    pub preview_content: String,
    pub preview_visible: bool,
    pub preview_scroll: usize,
    pub preview_is_md: bool,
    pub pdf_cache: Vec<(PathBuf, String)>,
    pub last_preview: Instant,
}

impl PreviewState {
    pub fn new(preview_visible: bool) -> Self {
        Self {
            preview_content: String::new(),
            preview_visible,
            preview_scroll: 0,
            preview_is_md: false,
            pdf_cache: Vec::new(),
            last_preview: Instant::now(),
        }
    }
}
pub struct EditorState {
    pub editor_buffer: String,
    pub editor_file: PathBuf,
    pub editor_cursor_line: usize,
    pub editor_cursor_col: usize,
    pub editor_scroll: usize,
    pub editor_modified: bool,
    pub editor_selection: Option<(usize, usize)>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            editor_buffer: String::new(),
            editor_file: PathBuf::new(),
            editor_cursor_line: 0,
            editor_cursor_col: 0,
            editor_scroll: 0,
            editor_modified: false,
            editor_selection: None,
        }
    }
}
pub struct CommandState {
    pub command_input: String,
    pub command_suggestion: Vec<crate::suggestions::Suggestion>,
    pub suggestion_index: usize,
    pub suggestion_scroll: usize,
    pub command_history: Vec<String>,
    pub command_history_index: usize,
    pub suggestion_providers: Vec<Box<dyn crate::suggestions::SuggestionProvider>>,
}

impl CommandState {
    pub fn new() -> Self {
        Self {
            command_input: String::new(),
            command_suggestion: Vec::new(),
            suggestion_index: 0,
            suggestion_scroll: 0,
            command_history: Vec::new(),
            command_history_index: 0,
            suggestion_providers: vec![
                Box::new(crate::suggestions::CommandSnippetProvider),
                Box::new(crate::suggestions::FdPathProvider),
            ],
        }
    }
}
pub struct BookmarkState {
    pub bookmarks: Vec<PathBuf>,
    pub bookmark_cursor: usize,
}

impl BookmarkState {
    pub fn new(bookmarks: Vec<PathBuf>) -> Self {
        Self {
            bookmarks,
            bookmark_cursor: 0,
        }
    }
}
pub struct GrepState {
    pub grep_matches: Vec<Match>,
    pub grep_cursor: usize,
    pub last_grep_pattern: String,
    pub last_grep_dir: PathBuf,
}

impl GrepState {
    pub fn new() -> Self {
        Self {
            grep_matches: Vec::new(),
            grep_cursor: 0,
            last_grep_pattern: String::new(),
            last_grep_dir: PathBuf::new(),
        }
    }
}
pub struct DiskUsageState {
    pub disk_usage_items: Vec<DiskItem>,
    pub disk_usage_cursor: usize,
    pub disk_usage_total: u64,
}

impl DiskUsageState {
    pub fn new() -> Self {
        Self {
            disk_usage_items: Vec::new(),
            disk_usage_cursor: 0,
            disk_usage_total: 0,
        }
    }
}
pub struct SearchState {
    pub search_query: String,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct ActionEntry {
    pub label: String,
    pub verb: String,
    pub is_separator: bool,
    pub indent: u32,
    pub cmd_id: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

pub struct FileClipboardState {
    pub mode: ClipboardMode,
    pub paths: Vec<PathBuf>,
}

impl FileClipboardState {
    pub fn new() -> Self {
        Self { mode: ClipboardMode::Copy, paths: Vec::new() }
    }
}

pub struct CreatePromptState {
    pub choice: usize,
}

impl CreatePromptState {
    pub fn new() -> Self {
        Self { choice: 0 }
    }
}

pub struct ActionState {
    pub entries: Vec<ActionEntry>,
    pub cursor: usize,
    pub offset: usize,
}

impl ActionState {
    pub fn new() -> Self {
        Self { entries: Vec::new(), cursor: 0, offset: 0 }
    }
}

#[derive(Clone)]
pub struct OpenWithEntry {
    pub name: String,
    pub exe_path: String,
}

pub struct OpenWithState {
    pub entries: Vec<OpenWithEntry>,
    pub cursor: usize,
}

impl OpenWithState {
    pub fn new() -> Self {
        Self { entries: Vec::new(), cursor: 0 }
    }
}

#[allow(dead_code)]
pub fn format_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", b / (1024.0 * 1024.0 * 1024.0))
    } else if b >= 1024.0 * 1024.0 {
        format!("{:.1} MB", b / (1024.0 * 1024.0))
    } else if b >= 1024.0 {
        format!("{:.1} KB", b / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
