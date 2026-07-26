use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::event::AppEvent;
use crate::indexer::InMemoryIndex;
use crate::state::*;
use std::sync::Arc;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NEW_CONSOLE: u32 = 0x00000010;

fn discover_shell_verbs() -> Vec<ActionEntry> {
    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();

    for (hkey, base) in &[
        (RegKey::predef(HKEY_CURRENT_USER), r"Software\Classes\Directory\shell"),
        (RegKey::predef(HKEY_LOCAL_MACHINE), r"SOFTWARE\Classes\Directory\shell"),
    ] {
        if let Ok(shell) = hkey.open_subkey_with_flags(base, KEY_READ) {
            for verb in shell.enum_keys().flatten() {
                if let Ok(vk) = shell.open_subkey_with_flags(&verb, KEY_READ) {
                    let label: String = vk
                        .get_value("MUIVerb")
                        .or_else(|_| vk.get_value(""))
                        .unwrap_or_else(|_| verb.clone());
                    if let Ok(cmd_key) = vk.open_subkey_with_flags("command", KEY_READ) {
                        if let Ok(command) = cmd_key.get_value::<String, _>("") {
                            if seen.insert(command.clone()) {
                                entries.push(ActionEntry {
                                    label,
                                    command,
                                    description: format!("Open folder in {}", verb),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    entries
}

fn build_actions() -> Vec<ActionEntry> {
    let mut actions = vec![
        ActionEntry {
            label: "Open in Terminal".into(),
            command: "cmd /k".into(),
            description: "Open cmd in current directory".into(),
        },
        ActionEntry {
            label: "Open in Explorer".into(),
            command: "explorer".into(),
            description: "Open folder in Windows Explorer".into(),
        },
        ActionEntry {
            label: "Open with default app".into(),
            command: "start".into(),
            description: "Open selected file with default program".into(),
        },
    ];
    actions.extend(discover_shell_verbs());
    actions
}

#[derive(PartialEq)]
pub enum AppMode {
    Normal,
    Command,
    Search,
    Breadcrumbs,
    BookMarks,
    GrepResults,
    DiskUsage,
    Viewer,
    Editor,
    ConfirmDelete,
    Help,
    FileInfo,
    Action,
}
pub fn editor_line_starts(buf: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, ch) in buf.char_indices() {
        if ch == '\n' {
            starts.push(i + 1);
        }
    }
    starts
}

pub fn cursor_byte_offset(buf: &str, line: usize, col: usize) -> usize {
    let starts = editor_line_starts(buf);
    let line = line.min(starts.len().saturating_sub(1));
    let line_start = starts[line];
    let rest = &buf[line_start..];
    let line_end = rest.find('\n').map(|i| line_start + i).unwrap_or(buf.len());
    let line_content = &buf[line_start..line_end];
    let col = col.min(line_content.chars().count());
    line_content
        .chars()
        .take(col)
        .map(|c| c.len_utf8())
        .sum::<usize>()
        + line_start
}

pub struct App {
    pub mode: AppMode,
    pub config: Config,
    pub index: Arc<InMemoryIndex>,
    pub nav: NavState,
    pub preview: PreviewState,
    pub editor: EditorState,
    pub cmd: CommandState,
    pub bookmarks: BookmarkState,
    pub grep: GrepState,
    pub du: DiskUsageState,
    pub search: SearchState,
    pub status_message: String,
    pub status_message_time: Option<Instant>,
    pub index_tx: mpsc::Sender<String>,
    pub index_rx: mpsc::Receiver<String>,
    pub save_progress: Option<(usize, usize)>,
    pub help_scroll: usize,
    pub action: ActionState,
}

impl Drop for App {
    fn drop(&mut self) {
        self.save_bookmarks();
    }
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];
const MAX_PDF_CACHE: usize = 50;

impl App {
    pub fn new() -> Self {
        let config = Config::load();

        let mut loaded_bookmarks = Vec::new();
        if let Ok(file) = File::open(&config.general.bookmarks_file) {
            let reader = BufReader::new(file);
            for line in reader.lines().flatten() {
                let path_buf = PathBuf::from(line.trim());
                if path_buf.exists() {
                    loaded_bookmarks.push(path_buf);
                }
            }
        }

        let index = Arc::new(InMemoryIndex::new());
        let (index_tx, index_rx) = mpsc::channel();

        let cmd_state = CommandState::new();

        let cwd = std::env::current_dir().unwrap_or_default();

        let mut app = Self {
            mode: AppMode::Normal,
            config,
            index,
            nav: NavState::new(cwd.clone()),
            preview: PreviewState::new(true),
            editor: EditorState::new(),
            cmd: cmd_state,
            bookmarks: BookmarkState::new(loaded_bookmarks),
            grep: GrepState::new(),
            du: DiskUsageState::new(),
            search: SearchState::new(),
            status_message: String::new(),
            status_message_time: None,
            index_tx,
            index_rx,
            save_progress: None,
            help_scroll: 0,
            action: ActionState::new(build_actions()),
        };

        app.nav.history = vec![cwd];
        app.nav.history_pos = 0;
        app.refresh_files();
        app.update_preview();
        app
    }

    pub fn is_image(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    pub fn is_pdf(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false)
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_message_time = Some(Instant::now());
    }

    pub fn handle_event(&mut self, event: &AppEvent) -> Option<()> {
        use AppMode::*;
        match self.mode {
            Normal => self.handle_normal(event),
            Command => self.handle_command(event),
            Search => self.handle_search(event),
            Breadcrumbs => self.handle_breadcrumbs(event),
            BookMarks => self.handle_bookmarks(event),
            GrepResults => self.handle_grep_results(event),
            DiskUsage => self.handle_disk_usage(event),
            Viewer => self.handle_viewer(event),
            Editor => self.handle_editor(event),
            ConfirmDelete => self.handle_confirm_delete(event),
            Help => self.handle_help(event),
            FileInfo => self.handle_file_info(event),
            Action => self.handle_action(event),
        }
    }

    fn handle_normal(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape => {
                if !self.nav.filter_input.is_empty() {
                    self.nav.filter_input.clear();
                    self.apply_filter();
                }
            }
            AppEvent::Delete => {
                self.mode = AppMode::ConfirmDelete
            }
            AppEvent::Char('q') => {
                if self.editor.editor_modified {
                    self.save_progress = Some((0, 1));
                    let _ = self.editor_save();
                    self.save_progress = Some((1, 1));
                    self.set_status(format!(
                        "Auto-saved: {} — press q again to exit",
                        self.editor
                            .editor_file
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("?")
                    ));
                    return None;
                }
                return Some(());
            }
            AppEvent::Up => {
                if self.nav.cursor_index == 0 && !self.nav.path_segments.is_empty() {
                    self.mode = AppMode::Breadcrumbs;
                    self.nav.path_cursor = self.nav.path_segments.len() - 1;
                } else {
                    self.move_cursor_up();
                }
            }
            AppEvent::Down => self.move_cursor_down(),
            AppEvent::Char(' ') => self.toggle_select(),
            AppEvent::Enter => {
                if !self.nav.files.is_empty() {
                    let target = self.nav.files[self.nav.cursor_index].clone();
                    let is_dir = target.is_dir()
                        || self.nav.dir_cache.get(&target).map(|e| e.is_dir).unwrap_or(false);
                    if is_dir {
                        self.push_history();
                        self.nav.current_dir = target;
                        self.nav.cursor_index = 0;
                        self.refresh_files();
                    } else if App::is_image(&target) {
                        let _ = std::process::Command::new("mspaint").arg(&target).spawn();
                    } else if App::is_pdf(&target) {
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", "start", "", &target.to_string_lossy()])
                            .spawn();
                    }
                }
            }
            AppEvent::Backspace => {
                if !self.nav.filter_input.is_empty() {
                    self.nav.filter_input.pop();
                    self.apply_filter();
                } else {
                    let parent = self.nav.current_dir.parent().and_then(|p| {
                        let b = p.to_path_buf();
                        if b == self.nav.current_dir { None } else { Some(b) }
                    }).unwrap_or(self.nav.current_dir.clone());
                    if parent != self.nav.current_dir {
                        self.push_history();
                        self.nav.current_dir = parent;
                        self.nav.cursor_index = 0;
                        self.refresh_files();
                    }
                }
            }
            AppEvent::Char(':') => {
                self.mode = AppMode::Command;
                self.cmd.command_input.clear();
                self.cmd.command_suggestion.clear();
            }
            AppEvent::Char('b') => self.toggle_bookmarks(),
            AppEvent::Char('s') => self.cycle_sort(),
            AppEvent::Char('S') => self.toggle_sort_order(),
            AppEvent::Char('B') => {
                self.mode = AppMode::BookMarks;
                self.bookmarks.bookmark_cursor = 0;
            }
            AppEvent::Char('.') => {
                self.nav.show_hidden = !self.nav.show_hidden;
                self.refresh_files();
                self.set_status(if self.nav.show_hidden {
                    "Hidden files: shown"
                } else {
                    "Hidden files: hidden"
                });
            }
            AppEvent::Char('p') => {
                self.preview.preview_visible = !self.preview.preview_visible;
            }
            AppEvent::Char('v') => {
                self.open_viewer();
            }
            AppEvent::F(1) | AppEvent::Char('?') => {
                self.help_scroll = 0;
                self.mode = AppMode::Help;
            }
            AppEvent::AltLeft => self.history_back(),
            AppEvent::AltRight => self.history_forward(),
            AppEvent::Ctrl('y') => {
                if !self.nav.files.is_empty() {
                    self.mode = AppMode::FileInfo;
                }
            }
            AppEvent::Ctrl('r') => {
                if !self.grep.last_grep_pattern.is_empty() {
                    let matches = crate::grep::Searcher::search_with_gitignore(
                        &self.grep.last_grep_dir,
                        &self.grep.last_grep_pattern,
                        &self.config.indexing.skip_dirs,
                        &self.config.indexing.skip_files,
                    );
                    self.grep.grep_matches = matches;
                    self.grep.grep_cursor = 0;
                    self.mode = AppMode::GrepResults;
                }
            }
            AppEvent::Ctrl('a') => {
                self.mode = AppMode::Action;
                self.action.cursor = 0;
            }
            AppEvent::Char(c) => {
                if let Some((done, total)) = self.save_progress {
                    if done == total {
                        self.save_progress = None;
                        return Some(());
                    }
                }
                self.nav.filter_input.push(*c);
                self.apply_filter();
            }
            _ => {
                if let Some((done, total)) = self.save_progress {
                    if done == total {
                        self.save_progress = None;
                        return Some(());
                    }
                }
            }
        }
        None
    }

    fn handle_command(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape => {
                self.mode = AppMode::Normal;
                self.cmd.command_input.clear();
                self.cmd.command_suggestion.clear();
            }
            AppEvent::Enter => {
                let input = self.cmd.command_input.trim().to_string();
                if !input.is_empty()
                    && (self.cmd.command_history.is_empty()
                        || self.cmd.command_history[self.cmd.command_history.len() - 1] != input)
                {
                    self.cmd.command_history.push(input.clone());
                    if self.cmd.command_history.len() > 50 {
                        self.cmd.command_history.remove(0);
                    }
                    self.cmd.command_history_index = self.cmd.command_history.len();
                }
                if input.starts_with(":find") {
                    if self.cmd.suggestion_index > 0 {
                        if let Some(suggestion) =
                            self.cmd.command_suggestion.get(self.cmd.suggestion_index)
                        {
                            let suggestion = suggestion.clone();
                            self.cmd.command_suggestion.clear();
                            self.cmd.command_input.clear();
                            self.cmd.suggestion_index = 0;
                            if suggestion.ends_with('/') {
                                let dir_path = PathBuf::from(suggestion.trim_end_matches('/'));
                                if dir_path.is_dir() {
                                    self.push_history();
                                    self.nav.current_dir = dir_path;
                                    self.nav.cursor_index = 0;
                                    self.refresh_files();
                                }
                            } else {
                                let file_path = PathBuf::from(&suggestion);
                                if file_path.is_file() {
                                    if let Some(parent) = file_path.parent() {
                                        self.push_history();
                                        self.nav.current_dir = parent.to_path_buf();
                                    }
                                    self.refresh_files();
                                    if let Ok(text) = fs::read_to_string(&file_path) {
                                        self.preview.preview_content = text;
                                        self.preview.preview_scroll = 0;
                                    }
                                    self.mode = AppMode::Viewer;
                                    return None;
                                }
                            }
                            self.mode = AppMode::Normal;
                            return None;
                        }
                    }
                    self.execute_command();
                    if self.mode == AppMode::Command {
                        self.mode = AppMode::Normal;
                    }
                    return None;
                } else if input == ":q" {
                    if self.editor.editor_modified {
                        self.save_progress = Some((0, 1));
                        let _ = self.editor_save();
                        self.save_progress = Some((1, 1));
                        self.set_status(format!(
                            "Auto-saved: {} — press q again to exit",
                            self.editor
                                .editor_file
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("?")
                        ));
                        self.mode = AppMode::Normal;
                        return None;
                    }
                    return Some(());
                } else {
                    match input.as_str() {
                        ":du" => {
                            self.calculate_disk_usage();
                            self.mode = AppMode::DiskUsage;
                        }
                        _ if input.starts_with(":grep") => {
                            let raw_args = input.trim_start_matches(":grep ").trim();
                            let pattern = raw_args.to_string();
                            if let Some(re_pat) = pattern.strip_prefix("re:") {
                                if regex::Regex::new(re_pat).is_err() {
                                    self.set_status(format!("Invalid regex: {}", pattern));
                                    self.cmd.command_suggestion.clear();
                                    self.mode = AppMode::Normal;
                                    return None;
                                }
                            }
                            if !pattern.is_empty() {
                                self.grep.last_grep_pattern = pattern.clone();
                                self.grep.last_grep_dir = self.nav.current_dir.clone();
                                let matches = crate::grep::Searcher::search_with_gitignore(
                                    &self.nav.current_dir,
                                    &pattern,
                                    &self.config.indexing.skip_dirs,
                                    &self.config.indexing.skip_files,
                                );
                                self.grep.grep_matches = matches;
                                self.grep.grep_cursor = 0;
                                self.mode = AppMode::GrepResults;
                            } else if !self.grep.last_grep_pattern.is_empty() {
                                let matches = crate::grep::Searcher::search_with_gitignore(
                                    &self.grep.last_grep_dir,
                                    &self.grep.last_grep_pattern,
                                    &self.config.indexing.skip_dirs,
                                    &self.config.indexing.skip_files,
                                );
                                self.grep.grep_matches = matches;
                                self.grep.grep_cursor = 0;
                                self.mode = AppMode::GrepResults;
                            } else {
                                self.mode = AppMode::Normal;
                            }
                        }
                        _ if input == ":rm" || input.starts_with(":rm ") => {
                            self.mode = AppMode::ConfirmDelete;
                        }
                        _ => {
                            self.execute_command();
                            self.mode = AppMode::Normal;
                        }
                    }
                    self.cmd.command_suggestion.clear();
                }
            }
            AppEvent::Char(c) => {
                self.cmd.command_input.push(*c);
                self.update_suggestion();
            }
            AppEvent::Backspace => {
                self.cmd.command_input.pop();
                self.update_suggestion();
            }
            AppEvent::Tab | AppEvent::Down => {
                if !self.cmd.command_suggestion.is_empty() {
                    self.cmd.suggestion_index =
                        (self.cmd.suggestion_index + 1) % self.cmd.command_suggestion.len();
                } else if !self.cmd.command_history.is_empty() {
                    if self.cmd.command_history_index < self.cmd.command_history.len() - 1 {
                        self.cmd.command_history_index += 1;
                        self.cmd.command_input =
                            self.cmd.command_history[self.cmd.command_history_index].clone();
                        self.update_suggestion();
                    } else {
                        self.cmd.command_history_index = self.cmd.command_history.len();
                        self.cmd.command_input.clear();
                    }
                }
            }
            AppEvent::Up => {
                if !self.cmd.command_suggestion.is_empty() {
                    self.cmd.suggestion_index = if self.cmd.suggestion_index == 0 {
                        self.cmd.command_suggestion.len() - 1
                    } else {
                        self.cmd.suggestion_index - 1
                    };
                } else if !self.cmd.command_history.is_empty() && self.cmd.command_history_index > 0
                {
                    self.cmd.command_history_index -= 1;
                    self.cmd.command_input =
                        self.cmd.command_history[self.cmd.command_history_index].clone();
                    self.update_suggestion();
                }
            }
            AppEvent::PageDown => {
                if !self.cmd.command_suggestion.is_empty() {
                    self.cmd.suggestion_index = (self.cmd.suggestion_index + 8)
                        .min(self.cmd.command_suggestion.len().saturating_sub(1));
                }
            }
            AppEvent::PageUp => {
                if !self.cmd.command_suggestion.is_empty() {
                    self.cmd.suggestion_index = self.cmd.suggestion_index.saturating_sub(8);
                }
            }
            _ => {}
        }
        None
    }

    fn handle_search(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape => {
                self.mode = AppMode::Normal;
                self.refresh_files();
            }
            AppEvent::Up => self.move_cursor_up(),
            AppEvent::Down => self.move_cursor_down(),
            AppEvent::Enter => {
                if !self.nav.files.is_empty() {
                    let target = self.nav.files[self.nav.cursor_index].clone();
                    if target.is_dir() {
                        self.push_history();
                        self.nav.current_dir = target;
                        self.mode = AppMode::Normal;
                        self.nav.cursor_index = 0;
                        self.refresh_files();
                    } else if let Some(parent) = target.parent() {
                        self.push_history();
                        self.nav.current_dir = parent.to_path_buf();
                        self.mode = AppMode::Normal;
                        self.refresh_files();
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn handle_breadcrumbs(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Down => {
                self.mode = AppMode::Normal;
                self.nav.cursor_index = 0;
            }
            AppEvent::Left => {
                if self.nav.path_cursor > 0 {
                    self.nav.path_cursor -= 1;
                }
            }
            AppEvent::Right => {
                if self.nav.path_cursor < self.nav.path_segments.len() - 1 {
                    self.nav.path_cursor += 1;
                }
            }
            AppEvent::Enter => {
                let target_dir = self.nav.path_segments[self.nav.path_cursor].clone();
                self.push_history();
                self.nav.current_dir = target_dir;
                self.nav.cursor_index = 0;
                self.refresh_files();
            }
            AppEvent::Escape => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
        None
    }

    fn handle_bookmarks(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape | AppEvent::Char('B') => {
                self.mode = AppMode::Normal;
            }
            AppEvent::Up => {
                if !self.bookmarks.bookmarks.is_empty() {
                    self.bookmarks.bookmark_cursor = if self.bookmarks.bookmark_cursor > 0 {
                        self.bookmarks.bookmark_cursor - 1
                    } else {
                        self.bookmarks.bookmarks.len() - 1
                    };
                }
            }
            AppEvent::Down => {
                if !self.bookmarks.bookmarks.is_empty() {
                    self.bookmarks.bookmark_cursor =
                        if self.bookmarks.bookmark_cursor < self.bookmarks.bookmarks.len() - 1 {
                            self.bookmarks.bookmark_cursor + 1
                        } else {
                            0
                        };
                }
            }
            AppEvent::Enter => {
                if !self.bookmarks.bookmarks.is_empty() {
                    let target_dir =
                        self.bookmarks.bookmarks[self.bookmarks.bookmark_cursor].clone();
                    self.push_history();
                    self.nav.current_dir = target_dir;
                    self.nav.cursor_index = 0;
                    self.refresh_files();
                    self.mode = AppMode::Normal;
                }
            }
            AppEvent::Char('d') | AppEvent::Delete => {
                if !self.bookmarks.bookmarks.is_empty()
                    && self.bookmarks.bookmark_cursor < self.bookmarks.bookmarks.len()
                {
                    self.bookmarks
                        .bookmarks
                        .remove(self.bookmarks.bookmark_cursor);
                    if self.bookmarks.bookmarks.is_empty() {
                        self.bookmarks.bookmark_cursor = 0;
                    } else if self.bookmarks.bookmark_cursor >= self.bookmarks.bookmarks.len() {
                        self.bookmarks.bookmark_cursor = self.bookmarks.bookmarks.len() - 1;
                    }
                    self.save_bookmarks();
                }
            }
            _ => {}
        }
        None
    }

    fn handle_grep_results(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape => {
                self.mode = AppMode::Normal;
            }
            AppEvent::Up => {
                if !self.grep.grep_matches.is_empty() {
                    self.grep.grep_cursor = if self.grep.grep_cursor > 0 {
                        self.grep.grep_cursor - 1
                    } else {
                        self.grep.grep_matches.len() - 1
                    };
                }
            }
            AppEvent::Down => {
                if !self.grep.grep_matches.is_empty() {
                    self.grep.grep_cursor =
                        if self.grep.grep_cursor < self.grep.grep_matches.len() - 1 {
                            self.grep.grep_cursor + 1
                        } else {
                            0
                        };
                }
            }
            AppEvent::Enter => {
                if !self.grep.grep_matches.is_empty() {
                    let m = &self.grep.grep_matches[self.grep.grep_cursor];
                    let file = m.file.clone();
                    let line = m.line;
                    if let Some(parent) = file.parent().map(|p| p.to_path_buf()) {
                        self.push_history();
                        self.nav.current_dir = parent;
                        self.refresh_files();
                        if let Some(index) = self.nav.files.iter().position(|x| x == &file) {
                            self.nav.cursor_index = index;
                        }
                    }
                    if let Ok(text) = fs::read_to_string(&file) {
                        self.preview.preview_content = text;
                        self.preview.preview_scroll = line.saturating_sub(16);
                        self.preview.preview_is_md = file
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("md"))
                            .unwrap_or(false);
                        self.mode = AppMode::Viewer;
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn handle_disk_usage(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape => {
                self.mode = AppMode::Normal;
            }
            AppEvent::Up => {
                if !self.du.disk_usage_items.is_empty() && self.du.disk_usage_cursor > 0 {
                    self.du.disk_usage_cursor -= 1;
                }
            }
            AppEvent::Down => {
                if !self.du.disk_usage_items.is_empty()
                    && self.du.disk_usage_cursor < self.du.disk_usage_items.len() - 1
                {
                    self.du.disk_usage_cursor += 1;
                }
            }
            AppEvent::Enter => {
                if !self.du.disk_usage_items.is_empty() {
                    let target = self.du.disk_usage_items[self.du.disk_usage_cursor].clone();
                    if target.is_dir {
                        self.push_history();
                        self.nav.current_dir = target.path;
                        self.calculate_disk_usage();
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn handle_viewer(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Up | AppEvent::Char('k') => {
                if self.preview.preview_scroll > 0 {
                    self.preview.preview_scroll -= 1;
                }
            }
            AppEvent::Down | AppEvent::Char('j') => {
                self.preview.preview_scroll += 1;
            }
            AppEvent::PageUp => {
                self.preview.preview_scroll = self.preview.preview_scroll.saturating_sub(20);
            }
            AppEvent::PageDown => {
                self.preview.preview_scroll += 20;
            }
            AppEvent::Home => {
                self.preview.preview_scroll = 0;
            }
            AppEvent::End => {
                self.preview.preview_scroll = usize::MAX;
            }
            AppEvent::Char('e') | AppEvent::Char('i') => {
                self.open_editor();
            }
            AppEvent::Escape | AppEvent::Char('q') | AppEvent::Char('v') => {
                self.mode = AppMode::Normal;
                self.refresh_files();
            }
            _ => {}
        }
        None
    }

    fn handle_editor(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape => {
                if self.editor.editor_modified {
                    let _ = self.editor_save();
                }
                self.mode = AppMode::Normal;
                self.refresh_files();
            }
            AppEvent::Up => self.editor_move_up(),
            AppEvent::Down => self.editor_move_down(),
            AppEvent::CtrlShiftLeft => {
                self.editor_select_word_left()
            }
            AppEvent::CtrlShiftRight => {
                self.editor_select_word_right()
            }
            AppEvent::CtrlLeft => self.editor_move_word_left(),
            AppEvent::CtrlRight => {
                self.editor_move_word_right()
            }
            AppEvent::Left => self.editor_move_left(),
            AppEvent::Right => self.editor_move_right(),
            AppEvent::Backspace => self.editor_backspace(),
            AppEvent::Delete => self.editor_delete(),
            AppEvent::Enter => self.editor_insert_newline(),
            AppEvent::Ctrl('s') => {
                if let Err(e) = self.editor_save() {
                    eprintln!("Save error: {}", e);
                }
            }
            AppEvent::Ctrl('x') => {
                self.editor_cut();
            }
            AppEvent::Ctrl('c') => {
                self.editor_copy();
            }
            AppEvent::Tab => self.editor_insert_char('\t'),
            AppEvent::Char(c) => self.editor_insert_char(*c),
            _ => {}
        }
        None
    }

    fn handle_help(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape | AppEvent::Char('q') | AppEvent::F(1) => {
                self.help_scroll = 0;
                self.mode = AppMode::Normal;
            }
            AppEvent::Up | AppEvent::Char('k') => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            AppEvent::Down | AppEvent::Char('j') => {
                self.help_scroll = self.help_scroll.saturating_add(1);
            }
            AppEvent::PageUp => {
                self.help_scroll = self.help_scroll.saturating_sub(20);
            }
            AppEvent::PageDown => {
                self.help_scroll = self.help_scroll.saturating_add(20);
            }
            AppEvent::Home => {
                self.help_scroll = 0;
            }
            AppEvent::End => {
                self.help_scroll = usize::MAX;
            }
            _ => {}
        }
        None
    }

    fn handle_file_info(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape | AppEvent::Char('q') | AppEvent::Char('y') => {
                self.mode = AppMode::Normal;
            }
            AppEvent::Ctrl('c') => {
                if let Some(path) = self.nav.files.get(self.nav.cursor_index) {
                    if let Ok(mut clip) = arboard::Clipboard::new() {
                        let _ = clip.set_text(path.to_string_lossy().to_string());
                        self.set_status("Path copied to clipboard");
                    }
                }
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
        None
    }

    fn handle_action(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Escape | AppEvent::Char('q') => {
                self.mode = AppMode::Normal;
            }
            AppEvent::Up => {
                if self.action.cursor > 0 {
                    self.action.cursor -= 1;
                    if self.action.cursor < self.action.offset {
                        self.action.offset = self.action.cursor;
                    }
                }
            }
            AppEvent::Down => {
                if self.action.cursor + 1 < self.action.actions.len() {
                    self.action.cursor += 1;
                }
            }
            AppEvent::Enter => {
                let cwd = &self.nav.current_dir;
                let Some(entry) = self.action.actions.get(self.action.cursor) else {
                    return None;
                };
                let cmd = &entry.command;
                let result = if cmd == "cmd /k" {
                    std::process::Command::new("cmd")
                        .arg("/k")
                        .current_dir(cwd)
                        .creation_flags(CREATE_NEW_CONSOLE)
                        .spawn()
                        .err()
                        .map(|e| format!("Failed to open terminal: {}", e))
                } else if cmd == "explorer" {
                    std::process::Command::new("explorer")
                        .arg(cwd.as_os_str())
                        .spawn()
                        .err()
                        .map(|e| format!("Failed to open explorer: {}", e))
                } else if cmd == "start" {
                    let target = self.nav.files.get(self.nav.cursor_index)
                        .cloned()
                        .unwrap_or_else(|| cwd.clone());
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", &target.to_string_lossy()])
                        .spawn();
                    None
                } else {
                    let target = self.nav.files.get(self.nav.cursor_index)
                        .cloned()
                        .unwrap_or_else(|| cwd.clone());
                    if cmd.contains("%1") {
                        let expanded = cmd.replace("%1", &target.to_string_lossy());
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", &expanded])
                            .creation_flags(CREATE_NEW_CONSOLE)
                            .spawn();
                    } else {
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", cmd, &target.to_string_lossy()])
                            .creation_flags(CREATE_NEW_CONSOLE)
                            .spawn();
                    }
                    None
                };
                if let Some(msg) = result {
                    self.set_status(msg);
                }
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
        None
    }

    fn handle_confirm_delete(&mut self, event: &AppEvent) -> Option<()> {
        match event {
            AppEvent::Char('y') | AppEvent::Char('Y') | AppEvent::Enter => {
                let to_delete: Vec<_> = if !self.nav.selected_files.is_empty() {
                    self.nav.selected_files.iter().cloned().collect()
                } else if !self.nav.files.is_empty() {
                    vec![self.nav.files[self.nav.cursor_index].clone()]
                } else {
                    Vec::new()
                };
                let _ = trash::delete_all(&to_delete);
                self.nav.selected_files.clear();
                self.refresh_files();
                self.mode = AppMode::Normal;
            }
            AppEvent::Char('n') | AppEvent::Char('N') | AppEvent::Escape => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
        None
    }

    pub fn save_bookmarks(&self) {
        if let Ok(mut file) = File::create(&self.config.general.bookmarks_file) {
            for bookmark in &self.bookmarks.bookmarks {
                let _ = writeln!(file, "{}", bookmark.display());
            }
        }
    }

    pub fn toggle_bookmarks(&mut self) {
        if !self.nav.files.is_empty() && self.nav.cursor_index < self.nav.files.len() {
            let selected_path = self.nav.files[self.nav.cursor_index].clone();
            if self.bookmarks.bookmarks.contains(&selected_path) {
                self.bookmarks.bookmarks.retain(|x| x != &selected_path);
            } else {
                self.bookmarks.bookmarks.push(selected_path);
            }
        }
        self.save_bookmarks();
    }

    pub fn try_recv_suggestions(&mut self) {
    }

    pub fn try_recv_index_status(&mut self) {
        while let Ok(msg) = self.index_rx.try_recv() {
            self.set_status(msg);
        }
    }


    pub fn push_history(&mut self) {
        self.nav.history.truncate(self.nav.history_pos + 1);
        self.nav.history.push(self.nav.current_dir.clone());
        self.nav.history_pos = self.nav.history.len() - 1;
    }

    pub fn history_back(&mut self) {
        if self.nav.history_pos > 0 {
            self.nav.history_pos -= 1;
            self.nav.current_dir = self.nav.history[self.nav.history_pos].clone();
            self.nav.cursor_index = 0;
            self.refresh_files();
        }
    }

    pub fn history_forward(&mut self) {
        if self.nav.history_pos + 1 < self.nav.history.len() {
            self.nav.history_pos += 1;
            self.nav.current_dir = self.nav.history[self.nav.history_pos].clone();
            self.nav.cursor_index = 0;
            self.refresh_files();
        }
    }

    pub fn refresh_files(&mut self) {
        let now = Instant::now();
        if let Some(&last) = self.nav.last_dir_refresh.get(&self.nav.current_dir) {
            if now.duration_since(last) < Duration::from_millis(500) {
                return;
            }
        }
        self.nav
            .last_dir_refresh
            .insert(self.nav.current_dir.clone(), now);

        self.nav.filter_input.clear();
        self.nav.files.clear();

        let mut segments = Vec::new();
        let mut current = Some(self.nav.current_dir.as_path());
        while let Some(p) = current {
            if p.file_name().is_some() || p.parent().is_none() {
                segments.push(p.to_path_buf());
            }
            let next = p.parent();
            if next == current {
                break;
            }
            current = next;
        }
        segments.reverse();
        self.nav.path_segments = segments;

        if let Ok(entries) = fs::read_dir(&self.nav.current_dir) {
            self.nav.dir_cache.clear();
            let mut all_files: Vec<PathBuf> = Vec::new();
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !self.nav.show_hidden {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') {
                            continue;
                        }
                    }
                }
                let meta = entry.metadata().ok();
                let is_dir = meta.as_ref().map(|m| m.is_dir())
                    .or_else(|| entry.file_type().ok().map(|t| t.is_dir()))
                    .unwrap_or(false);
                let modified = meta.as_ref().and_then(|m| m.modified().ok());
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                self.nav.dir_cache.insert(
                    path.clone(),
                    DirEntry {
                        is_dir,
                        modified,
                        size,
                    },
                );
                all_files.push(path);
            }
            Self::sort_entries(
                &mut all_files,
                &self.nav.dir_cache,
                self.nav.sort_mode,
                self.nav.sort_reverse,
            );
            self.nav.files = all_files;
        }

        if self.nav.cursor_index >= self.nav.files.len() && !self.nav.files.is_empty() {
            self.nav.cursor_index = self.nav.files.len() - 1;
        }
        self.nav.full_files = self.nav.files.clone();
    }

    fn sort_entries(
        entries: &mut Vec<PathBuf>,
        cache: &HashMap<PathBuf, DirEntry>,
        sort_mode: SortMode,
        sort_reverse: bool,
    ) {
        entries.sort_by(|a, b| {
            let a_entry = cache.get(a);
            let b_entry = cache.get(b);
            let a_is_dir = a_entry.map(|e| e.is_dir).unwrap_or(false);
            let b_is_dir = b_entry.map(|e| e.is_dir).unwrap_or(false);
            let dir_cmp = match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };
            if dir_cmp != std::cmp::Ordering::Equal {
                return if sort_reverse {
                    dir_cmp.reverse()
                } else {
                    dir_cmp
                };
            }
            let cmp = match sort_mode {
                SortMode::ByName => a.file_name().cmp(&b.file_name()),
                SortMode::ByDate => a_entry
                    .and_then(|e| e.modified)
                    .cmp(&b_entry.and_then(|e| e.modified)),
                SortMode::BySize => a_entry
                    .map(|e| e.size)
                    .unwrap_or(0)
                    .cmp(&b_entry.map(|e| e.size).unwrap_or(0)),
                SortMode::ByType => a
                    .extension()
                    .unwrap_or_default()
                    .cmp(&b.extension().unwrap_or_default()),
            };
            if sort_reverse { cmp.reverse() } else { cmp }
        });
    }

    pub fn cycle_sort(&mut self) {
        self.nav.sort_mode = match self.nav.sort_mode {
            SortMode::ByName => SortMode::ByDate,
            SortMode::ByDate => SortMode::BySize,
            SortMode::BySize => SortMode::ByType,
            SortMode::ByType => SortMode::ByName,
        };
        self.sort_cached();
    }

    pub fn toggle_sort_order(&mut self) {
        self.nav.sort_reverse = !self.nav.sort_reverse;
        self.sort_cached();
    }

    fn sort_cached(&mut self) {
        if self.nav.dir_cache.is_empty() {
            return;
        }
        Self::sort_entries(
            &mut self.nav.files,
            &self.nav.dir_cache,
            self.nav.sort_mode,
            self.nav.sort_reverse,
        );
    }

    pub fn apply_filter(&mut self) {
        if self.nav.filter_input.is_empty() {
            self.nav.files = self.nav.full_files.clone();
        } else {
            let query = self.nav.filter_input.to_lowercase();
            self.nav.files = self
                .nav
                .full_files
                .iter()
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.to_lowercase().contains(&query))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
        }
        self.nav.cursor_index = 0;
        self.update_preview();
    }

    fn entry_size(&self, path: &Path) -> u64 {
        self.nav
            .dir_cache
            .get(path)
            .map(|e| e.size)
            .unwrap_or_else(|| path.metadata().map(|m| m.len()).unwrap_or(0))
    }

    pub fn move_cursor_up(&mut self) {
        if self.nav.files.is_empty() {
            return;
        }
        self.nav.cursor_index = if self.nav.cursor_index > 0 {
            self.nav.cursor_index - 1
        } else {
            self.nav.files.len() - 1
        };
        self.update_preview();
    }

    pub fn move_cursor_down(&mut self) {
        if self.nav.files.is_empty() {
            return;
        }
        self.nav.cursor_index = if self.nav.cursor_index + 1 < self.nav.files.len() {
            self.nav.cursor_index + 1
        } else {
            0
        };
        self.update_preview();
    }

    pub fn toggle_select(&mut self) {
        if !self.nav.files.is_empty() {
            let current_file = self.nav.files[self.nav.cursor_index].clone();
            if self.nav.selected_files.contains(&current_file) {
                self.nav.selected_files.remove(&current_file);
            } else {
                self.nav.selected_files.insert(current_file);
            }
        }
    }

    pub fn update_preview(&mut self) {
        if self.preview.last_preview.elapsed() < Duration::from_millis(80) {
            return;
        }
        self.preview.last_preview = Instant::now();
        self.preview.preview_content.clear();
        self.preview.preview_is_md = false;

        if self.nav.files.is_empty() {
            return;
        }
        let path = &self.nav.files[self.nav.cursor_index];

        // Folder
        if self
            .nav
            .dir_cache
            .get(path)
            .map(|e| e.is_dir)
            .unwrap_or_else(|| path.is_dir())
        {
            let count = self
                .nav
                .dir_child_count
                .get(path)
                .copied()
                .unwrap_or_else(|| {
                    let c = fs::read_dir(path).map(|e| e.count()).unwrap_or(0);
                    self.nav.dir_child_count.insert(path.to_path_buf(), c);
                    c
                });
            self.preview.preview_content =
                format!("Folder\n\n {}\n\n {} items", path.display(), count);
            return;
        }

        // Image
        if Self::is_image(path) {
            let len = self.entry_size(path);
            self.preview.preview_content = format!(
                "Image | {}\n\n[Enter] — open in mspaint",
                format_size(len)
            );
            return;
        }

        // PDF
        if Self::is_pdf(path) {
            let len = self.entry_size(path);
            let size_str = format_size(len);
            if let Some(pos) = self.preview.pdf_cache.iter().position(|(p, _)| p == path) {
                self.preview.preview_content = self.preview.pdf_cache[pos].1.clone();
                return;
            }
            self.preview.preview_content = match lopdf::Document::load(path) {
                Ok(doc) => {
                    let total_pages = doc.get_pages().len();
                    let mut content = format!("PDF | {} pages | {}\n\n", total_pages, size_str);
                    let page_nums: Vec<u32> = (1..=5.min(total_pages as u32)).collect();
                    match doc.extract_text(&page_nums) {
                        Ok(text) if !text.trim().is_empty() => {
                            let truncated: String =
                                text.lines().take(80).collect::<Vec<_>>().join("\n");
                            content.push_str(&truncated);
                            if text.lines().count() > 80 {
                                content.push_str("\n\n... (more lines)");
                            }
                        }
                        _ => content.push_str("[text not extracted — possibly scanned PDF]"),
                    }
                    if self.preview.pdf_cache.len() >= MAX_PDF_CACHE {
                        self.preview.pdf_cache.remove(0);
                    }
                    self.preview.pdf_cache.push((path.clone(), content.clone()));
                    content
                }
                Err(_) => format!("PDF | {}\n\n[error loading file]", size_str),
            };
            return;
        }

        // Markdown
        let is_md = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if is_md {
            if let Ok(text) = fs::read_to_string(path) {
                self.preview.preview_is_md = true;
                self.preview.preview_content = text;
            }
            return;
        }

        // Plain text (BufReader: read only first 20 lines)
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            let lines: Vec<String> = reader.lines().take(21).filter_map(|l| l.ok()).collect();
            let has_more = lines.len() > 20;
            let display: Vec<&str> = lines.iter().take(20).map(|s| s.as_str()).collect();
            self.preview.preview_content = display.join("\n");
            if has_more {
                self.preview
                    .preview_content
                    .push_str("\n\n ... (more lines)");
            }
        } else {
            let len = self.entry_size(path);
            self.preview.preview_content = format!("[BINARY] {} bytes ({})", len, format_size(len));
        }
    }

    pub fn open_viewer(&mut self) {
        if self.nav.files.is_empty() {
            return;
        }
        let path = self.nav.files[self.nav.cursor_index].clone();
        if path.is_file() {
            if let Ok(text) = fs::read_to_string(&path) {
                self.preview.preview_content = text;
                self.preview.preview_scroll = 0;
                self.mode = AppMode::Viewer;
            }
            self.preview.preview_is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
        }
    }

    pub fn open_editor(&mut self) {
        if self.nav.files.is_empty() {
            return;
        }
        self.editor.editor_file = self.nav.files[self.nav.cursor_index].clone();
        self.editor.editor_buffer = self.preview.preview_content.clone();
        self.editor.editor_cursor_line = self.preview.preview_scroll;
        self.editor.editor_cursor_col = 0;
        self.editor.editor_scroll = self.preview.preview_scroll;
        self.editor.editor_modified = false;
        self.editor.editor_selection = None;
        self.mode = AppMode::Editor;
    }

    pub fn editor_move_up(&mut self) {
        self.editor.editor_selection = None;
        if self.editor.editor_cursor_line > 0 {
            self.editor.editor_cursor_line -= 1;
            let line_len = self.editor_line_length(self.editor.editor_cursor_line);
            self.editor.editor_cursor_col = self.editor.editor_cursor_col.min(line_len);
        }
    }

    pub fn editor_move_down(&mut self) {
        self.editor.editor_selection = None;
        let total = self.editor_line_count();
        if self.editor.editor_cursor_line + 1 < total {
            self.editor.editor_cursor_line += 1;
            let line_len = self.editor_line_length(self.editor.editor_cursor_line);
            self.editor.editor_cursor_col = self.editor.editor_cursor_col.min(line_len);
        }
    }

    pub fn editor_move_left(&mut self) {
        self.editor.editor_selection = None;
        if self.editor.editor_cursor_col > 0 {
            self.editor.editor_cursor_col -= 1;
        } else if self.editor.editor_cursor_line > 0 {
            self.editor.editor_cursor_line -= 1;
            self.editor.editor_cursor_col = self.editor_line_length(self.editor.editor_cursor_line);
        }
    }

    pub fn editor_move_right(&mut self) {
        self.editor.editor_selection = None;
        let line_len = self.editor_line_length(self.editor.editor_cursor_line);
        if self.editor.editor_cursor_col < line_len {
            self.editor.editor_cursor_col += 1;
        } else {
            let total = self.editor_line_count();
            if self.editor.editor_cursor_line + 1 < total {
                self.editor.editor_cursor_line += 1;
                self.editor.editor_cursor_col = 0;
            }
        }
    }

    pub fn editor_insert_char(&mut self, c: char) {
        let offset = cursor_byte_offset(
            &self.editor.editor_buffer,
            self.editor.editor_cursor_line,
            self.editor.editor_cursor_col,
        );
        self.editor.editor_buffer.insert(offset, c);
        self.editor.editor_cursor_col += 1;
        self.editor.editor_modified = true;
    }

    pub fn editor_insert_newline(&mut self) {
        let offset = cursor_byte_offset(
            &self.editor.editor_buffer,
            self.editor.editor_cursor_line,
            self.editor.editor_cursor_col,
        );
        self.editor.editor_buffer.insert(offset, '\n');
        self.editor.editor_cursor_line += 1;
        self.editor.editor_cursor_col = 0;
        self.editor.editor_modified = true;
    }

    pub fn editor_backspace(&mut self) {
        if self.editor.editor_selection.is_some() {
            self.editor_delete_selection();
            return;
        }
        if self.editor.editor_cursor_col > 0 {
            let offset = cursor_byte_offset(
                &self.editor.editor_buffer,
                self.editor.editor_cursor_line,
                self.editor.editor_cursor_col,
            );
            if let Some(ch) = self.editor.editor_buffer[..offset].chars().last() {
                self.editor
                    .editor_buffer
                    .replace_range(offset - ch.len_utf8()..offset, "");
                self.editor.editor_cursor_col -= 1;
                self.editor.editor_modified = true;
            }
        } else if self.editor.editor_cursor_line > 0 {
            let prev_line_len = self.editor_line_length(self.editor.editor_cursor_line - 1);
            let offset = cursor_byte_offset(
                &self.editor.editor_buffer,
                self.editor.editor_cursor_line,
                0,
            );
            self.editor.editor_buffer.remove(offset - 1);
            self.editor.editor_cursor_line -= 1;
            self.editor.editor_cursor_col = prev_line_len;
            self.editor.editor_modified = true;
        }
    }

    pub fn editor_delete(&mut self) {
        if self.editor.editor_selection.is_some() {
            self.editor_delete_selection();
            return;
        }
        let offset = cursor_byte_offset(
            &self.editor.editor_buffer,
            self.editor.editor_cursor_line,
            self.editor.editor_cursor_col,
        );
        if offset < self.editor.editor_buffer.len() {
            let ch = self.editor.editor_buffer[offset..].chars().next().unwrap();
            self.editor
                .editor_buffer
                .replace_range(offset..offset + ch.len_utf8(), "");
            self.editor.editor_modified = true;
            if ch == '\n' {
                let line_len = self.editor_line_length(self.editor.editor_cursor_line);
                self.editor.editor_cursor_col = self.editor.editor_cursor_col.min(line_len);
            }
        }
    }

    pub fn editor_move_word_left(&mut self) {
        self.editor.editor_selection = None;
        self.editor_move_word_left_raw();
    }

    fn editor_move_word_left_raw(&mut self) {
        let line = self.editor_line_content(self.editor.editor_cursor_line);
        if self.editor.editor_cursor_col == 0 {
            if self.editor.editor_cursor_line > 0 {
                self.editor.editor_cursor_line -= 1;
                self.editor.editor_cursor_col =
                    self.editor_line_length(self.editor.editor_cursor_line);
            }
            return;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.editor.editor_cursor_col.min(chars.len());
        while col > 0 && chars[col - 1].is_whitespace() {
            col -= 1;
        }
        while col > 0 && !chars[col - 1].is_whitespace() {
            col -= 1;
        }
        self.editor.editor_cursor_col = col;
    }

    pub fn editor_move_word_right(&mut self) {
        self.editor.editor_selection = None;
        self.editor_move_word_right_raw();
    }

    fn editor_move_word_right_raw(&mut self) {
        let line = self.editor_line_content(self.editor.editor_cursor_line);
        let chars: Vec<char> = line.chars().collect();
        let total = chars.len();
        if self.editor.editor_cursor_col >= total {
            let total_lines = self.editor_line_count();
            if self.editor.editor_cursor_line + 1 < total_lines {
                self.editor.editor_cursor_line += 1;
                self.editor.editor_cursor_col = 0;
            }
            return;
        }
        let mut col = self.editor.editor_cursor_col;
        while col < total && !chars[col].is_whitespace() {
            col += 1;
        }
        while col < total && chars[col].is_whitespace() {
            col += 1;
        }
        self.editor.editor_cursor_col = col;
    }

    pub fn editor_select_word_left(&mut self) {
        if self.editor.editor_selection.is_none() {
            self.editor.editor_selection = Some((
                self.editor.editor_cursor_line,
                self.editor.editor_cursor_col,
            ));
        }
        self.editor_move_word_left_raw();
    }

    pub fn editor_select_word_right(&mut self) {
        if self.editor.editor_selection.is_none() {
            self.editor.editor_selection = Some((
                self.editor.editor_cursor_line,
                self.editor.editor_cursor_col,
            ));
        }
        self.editor_move_word_right_raw();
    }

    fn editor_selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        self.editor.editor_selection.map(|anchor| {
            let cursor = (
                self.editor.editor_cursor_line,
                self.editor.editor_cursor_col,
            );
            if (anchor.0, anchor.1) < (cursor.0, cursor.1) {
                (anchor, cursor)
            } else {
                (cursor, anchor)
            }
        })
    }

    pub fn editor_selection_text(&self) -> String {
        if let Some(((sl, sc), (el, ec))) = self.editor_selection_range() {
            let start = cursor_byte_offset(&self.editor.editor_buffer, sl, sc);
            let end = cursor_byte_offset(&self.editor.editor_buffer, el, ec);
            self.editor.editor_buffer[start..end].to_string()
        } else {
            String::new()
        }
    }

    pub fn editor_delete_selection(&mut self) {
        if let Some(((sl, sc), (el, ec))) = self.editor_selection_range() {
            let start = cursor_byte_offset(&self.editor.editor_buffer, sl, sc);
            let end = cursor_byte_offset(&self.editor.editor_buffer, el, ec);
            self.editor.editor_buffer.drain(start..end);
            self.editor.editor_cursor_line = sl;
            self.editor.editor_cursor_col = sc;
            self.editor.editor_selection = None;
            self.editor.editor_modified = true;
        }
    }

    pub fn editor_copy(&mut self) {
        let text = self.editor_selection_text();
        if !text.is_empty() {
            if let Ok(mut clip) = arboard::Clipboard::new() {
                let _ = clip.set_text(text);
            }
        }
    }

    pub fn editor_cut(&mut self) {
        let text = self.editor_selection_text();
        if !text.is_empty() {
            if let Ok(mut clip) = arboard::Clipboard::new() {
                let _ = clip.set_text(text);
            }
        }
        self.editor_delete_selection();
    }

    pub fn editor_line_content(&self, line: usize) -> &str {
        let starts = editor_line_starts(&self.editor.editor_buffer);
        if line >= starts.len() {
            return "";
        }
        let start = starts[line];
        let end = self.editor.editor_buffer[start..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(self.editor.editor_buffer.len());
        &self.editor.editor_buffer[start..end]
    }

    pub fn editor_save(&mut self) -> Result<(), std::io::Error> {
        fs::write(&self.editor.editor_file, &self.editor.editor_buffer)?;
        self.editor.editor_modified = false;
        Ok(())
    }

    fn editor_line_count(&self) -> usize {
        self.editor
            .editor_buffer
            .chars()
            .filter(|&c| c == '\n')
            .count()
            + 1
    }

    fn editor_line_length(&self, line: usize) -> usize {
        let starts = editor_line_starts(&self.editor.editor_buffer);
        if line >= starts.len() {
            return 0;
        }
        let start = starts[line];
        let end = self.editor.editor_buffer[start..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(self.editor.editor_buffer.len());
        self.editor.editor_buffer[start..end].chars().count()
    }

    pub fn update_suggestion(&mut self) {
        self.cmd.suggestion_index = 0;
        self.cmd.suggestion_scroll = 0;

        let parts: Vec<&str> = self.cmd.command_input.split_whitespace().collect();
        if parts.len() < 2 {
            self.cmd.command_suggestion.clear();
            return;
        }

        let command = parts[0];
        let args_part = parts[1..].join(" ");

        if command == ":find" {
            if args_part.is_empty() {
                self.cmd.command_suggestion.clear();
                return;
            }
            if !self.index.is_built() && !self.index.is_building() {
                let idx = self.index.clone();
                let skip = self.config.indexing.skip_dirs.clone();
                idx.ensure_built(
                    self.nav.current_dir.clone(),
                    skip,
                    self.index_tx.clone(),
                );
            }
            if !self.index.is_built() {
                self.cmd.command_suggestion.clear();
                return;
            }
            let mut combined = self.nav.current_dir.to_string_lossy().to_lowercase().replace('\\', "/");
            if !combined.ends_with('/') {
                combined.push('/');
            }
            let query_norm = args_part.to_lowercase().replace('\\', "/");
            combined.push_str(&query_norm);
            let results = self.index.find_completions(combined.as_bytes(), 50);
            self.cmd.command_suggestion = results;
            return;
        }

        self.cmd.command_suggestion.clear();
        if command != "cd" && command != "cp" && command != "mv" && command != "rm" {
            return;
        }

        let args_lower = args_part.to_lowercase();
        for path in &self.nav.files {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.to_lowercase().starts_with(&args_lower) {
                    let is_dir = self
                        .nav
                        .dir_cache
                        .get(path)
                        .map(|e| e.is_dir)
                        .unwrap_or(false);
                    self.cmd.command_suggestion.push(if is_dir {
                        format!("{}/", name)
                    } else {
                        name.to_string()
                    });
                }
            }
        }
    }

    pub fn calculate_disk_usage(&mut self) {
        self.du.disk_usage_items.clear();
        self.du.disk_usage_total = 0;
        self.du.disk_usage_cursor = 0;

        let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();
        let mut file_sizes: HashMap<PathBuf, u64> = HashMap::new();
        let root = self.nav.current_dir.clone();

        let walker = ignore::WalkBuilder::new(&root)
            .hidden(false)
            .git_ignore(true)
            .build();

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path().to_path_buf();
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                if let Ok(meta) = entry.metadata() {
                    let size = meta.len();
                    file_sizes.insert(path.clone(), size);
                    let mut p = path.parent();
                    while let Some(parent) = p {
                        if parent == root {
                            break;
                        }
                        *dir_sizes.entry(parent.to_path_buf()).or_insert(0) += size;
                        p = parent.parent();
                    }
                }
            }
        }

        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string();
                let is_dir = path.is_dir();
                let size = if is_dir {
                    dir_sizes.get(&path).copied().unwrap_or(0)
                } else {
                    file_sizes.get(&path).copied().unwrap_or(0)
                };
                self.du.disk_usage_total += size;
                self.du.disk_usage_items.push(DiskItem {
                    path,
                    name,
                    is_dir,
                    size,
                });
            }
        }

        self.du.disk_usage_items.sort_by(|a, b| b.size.cmp(&a.size));
    }

    pub fn execute_command(&mut self) {
        let input = self.cmd.command_input.clone();
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            return;
        }
        let command = parts[0].strip_prefix(':').unwrap_or(parts[0]);
        let args = &parts[1..];

        match command {
            "mkdir" => {
                if let Some(name) = args.first() {
                    match fs::create_dir(self.nav.current_dir.join(name)) {
                        Ok(_) => self.set_status(format!("Created directory '{}'", name)),
                        Err(e) => self.set_status(format!("mkdir error: {}", e)),
                    }
                } else {
                    self.set_status("Usage: :mkdir <name>");
                }
            }
            "touch" => {
                if let Some(name) = args.first() {
                    match File::create(self.nav.current_dir.join(name)) {
                        Ok(_) => self.set_status(format!("Created file '{}'", name)),
                        Err(e) => self.set_status(format!("touch error: {}", e)),
                    }
                } else {
                    self.set_status("Usage: :touch <name>");
                }
            }
            "cd" => {
                if let Some(target) = args.first() {
                    let new_path = if *target == ".." {
                        self.nav
                            .current_dir
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| self.nav.current_dir.clone())
                    } else {
                        self.nav.current_dir.join(target)
                    };
                    if new_path.is_dir() {
                        self.nav.current_dir = new_path;
                        self.nav.cursor_index = 0;
                    } else {
                        self.set_status(format!("Directory not found: {}", target));
                    }
                }
            }
            "cp" => {
                if !self.nav.selected_files.is_empty() {
                    let paths: Vec<_> = self.nav.selected_files.iter().cloned().collect();
                    for file_path in &paths {
                        if let Some(file_name) = file_path.file_name() {
                            let dest_file = self.nav.current_dir.join(file_name);
                            if file_path != &dest_file && file_path.is_file() {
                                let _ = fs::copy(file_path, &dest_file);
                            }
                        }
                    }
                    self.nav.selected_files.clear();
                    self.set_status(format!("Copied {} file(s)", paths.len()));
                } else {
                    self.set_status("No files selected. Use Space to select.");
                }
            }
            "mv" => {
                if !self.nav.selected_files.is_empty() {
                    let paths: Vec<_> = self.nav.selected_files.iter().cloned().collect();
                    for file_path in &paths {
                        if let Some(file_name) = file_path.file_name() {
                            let dest_path = self.nav.current_dir.join(file_name);
                            if file_path != &dest_path {
                                let _ = fs::rename(file_path, &dest_path);
                            }
                        }
                    }
                    self.nav.selected_files.clear();
                } else if args.len() == 1 && !self.nav.files.is_empty() {
                    let from = &self.nav.files[self.nav.cursor_index];
                    let to = self.nav.current_dir.join(args[0]);
                    if let Err(e) = fs::rename(from, &to) {
                        self.set_status(format!("Rename error: {}", e));
                    }
                }
            }
            "rn" => {
                let new_name = args.join(" ");
                if new_name.is_empty() {
                    self.set_status("Usage: :rn <new_name>");
                    return;
                }
                let source = if !self.nav.selected_files.is_empty() {
                    self.nav.selected_files.iter().next().unwrap().clone()
                } else if !self.nav.files.is_empty() {
                    self.nav.files[self.nav.cursor_index].clone()
                } else {
                    self.set_status("No file selected");
                    return;
                };
                let target = self.nav.current_dir.join(&new_name);
                if target.exists() {
                    self.set_status(format!("Target '{}' already exists", new_name));
                } else if let Err(e) = fs::rename(&source, &target) {
                    self.set_status(format!("Rename error: {}", e));
                } else {
                    self.nav.selected_files.clear();
                    self.set_status(format!("Renamed to '{}'", new_name));
                }
            }
            "find" => {
                let query = args.join(" ");
                if !query.is_empty() {
                    let full_path = self.nav.current_dir.join(&query);
                    if full_path.is_dir() {
                        self.push_history();
                        self.nav.current_dir = full_path;
                        self.nav.cursor_index = 0;
                        self.refresh_files();
                        self.mode = AppMode::Normal;
                        return;
                    }
                    if full_path.is_file() {
                        if let Ok(text) = fs::read_to_string(&full_path) {
                            self.push_history();
                            if let Some(parent) = full_path.parent() {
                                self.nav.current_dir = parent.to_path_buf();
                            }
                            self.refresh_files();
                            self.preview.preview_content = text;
                            self.preview.preview_scroll = 0;
                            self.mode = AppMode::Viewer;
                            return;
                        }
                    }
                    if !self.index.is_built() && !self.index.is_building() {
                        let idx = self.index.clone();
                        let skip = self.config.indexing.skip_dirs.clone();
                        idx.ensure_built(
                            self.nav.current_dir.clone(),
                            skip,
                            self.index_tx.clone(),
                        );
                    }
                    if !self.index.is_built() {
                        self.set_status("Still indexing... wait and retry :find");
                        return;
                    }
                    self.search.search_query = query.clone();
                    let results = self.index.search(&query, &self.nav.current_dir, 50);
                    let count = results.len();
                    self.nav.files = results;
                    self.nav.cursor_index = 0;
                    self.mode = AppMode::Search;
                    self.cmd.command_suggestion.clear();
                    self.set_status(format!("Found {} result(s) for '{}'", count, query));
                    return;
                }
            }
            _ => {}
        }
        self.cmd.command_input.clear();
        self.refresh_files();
    }
}
