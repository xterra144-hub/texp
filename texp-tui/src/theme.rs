use ratatui::prelude::*;
use serde::Deserialize;

// ─── helpers ───────────────────────────────────────────────────────────────

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    match s.to_lowercase().as_str() {
        "reset" | "default" => return Some(Color::Reset),
        "black" => return Some(Color::Black),
        "red" => return Some(Color::Red),
        "green" => return Some(Color::Green),
        "yellow" => return Some(Color::Yellow),
        "blue" => return Some(Color::Blue),
        "magenta" => return Some(Color::Magenta),
        "cyan" => return Some(Color::Cyan),
        "white" => return Some(Color::White),
        "gray" | "grey" => return Some(Color::Gray),
        "darkgray" | "darkgrey" => return Some(Color::DarkGray),
        "lightred" => return Some(Color::LightRed),
        "lightgreen" => return Some(Color::LightGreen),
        "lightyellow" => return Some(Color::LightYellow),
        "lightblue" => return Some(Color::LightBlue),
        "lightmagenta" => return Some(Color::LightMagenta),
        "lightcyan" => return Some(Color::LightCyan),
        _ => {}
    }
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                let r = ((rgb >> 16) & 0xFF) as u8;
                let g = ((rgb >> 8) & 0xFF) as u8;
                let b = (rgb & 0xFF) as u8;
                return Some(Color::Rgb(r, g, b));
            }
        }
    }
    None
}

fn named_modifier(s: &str) -> Option<Modifier> {
    match s.to_lowercase().as_str() {
        "bold" => Some(Modifier::BOLD),
        "dim" => Some(Modifier::DIM),
        "italic" => Some(Modifier::ITALIC),
        "underlined" | "underline" => Some(Modifier::UNDERLINED),
        "slow_blink" | "slowblink" => Some(Modifier::SLOW_BLINK),
        "rapid_blink" | "rapidblink" => Some(Modifier::RAPID_BLINK),
        "reversed" | "reverse" => Some(Modifier::REVERSED),
        "hidden" => Some(Modifier::HIDDEN),
        "crossed_out" | "crossedout" | "strikethrough" => Some(Modifier::CROSSED_OUT),
        _ => None,
    }
}

fn parse_modifier(s: &str) -> Modifier {
    let mut m = Modifier::empty();
    for part in s.split(',') {
        if let Some(modi) = named_modifier(part.trim()) {
            m |= modi;
        }
    }
    m
}

// ─── ThemeColor ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ThemeColor(pub Color);

impl ThemeColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Color::Rgb(r, g, b))
    }
}

impl Default for ThemeColor {
    fn default() -> Self {
        Self(Color::Reset)
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match parse_color(&s) {
            Some(c) => ThemeColor(c),
            None => {
                eprintln!("[texp] unknown color '{}', using default", s);
                ThemeColor::default()
            }
        })
    }
}

// ─── ThemeModifier ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ThemeModifier(pub Modifier);

impl Default for ThemeModifier {
    fn default() -> Self {
        Self(Modifier::empty())
    }
}

impl<'de> Deserialize<'de> for ThemeModifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(ThemeModifier(parse_modifier(&s)))
    }
}

// ─── Icons ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Icons {
    #[serde(default = "default_icon_folder")]
    pub folder: String,
    #[serde(default = "default_icon_file")]
    pub file: String,
    #[serde(default = "default_icon_bookmarked")]
    pub bookmarked: String,
    #[serde(default = "default_icon_cursor")]
    pub cursor: String,
    #[serde(default = "default_icon_selected_prefix")]
    pub selected_prefix: String,
    #[serde(default = "default_icon_progress_filled")]
    pub progress_filled: String,
    #[serde(default = "default_icon_progress_empty")]
    pub progress_empty: String,
    #[serde(default = "default_icon_separator")]
    pub separator: String,
}

fn default_icon_folder() -> String { "📁".into() }
fn default_icon_file() -> String { "📄".into() }
fn default_icon_bookmarked() -> String { "⭐".into() }
fn default_icon_cursor() -> String { "👉 ".into() }
fn default_icon_selected_prefix() -> String { "[X] ".into() }
fn default_icon_progress_filled() -> String { "█".into() }
fn default_icon_progress_empty() -> String { "░".into() }
fn default_icon_separator() -> String { " > ".into() }

impl Default for Icons {
    fn default() -> Self {
        Self {
            folder: default_icon_folder(),
            file: default_icon_file(),
            bookmarked: default_icon_bookmarked(),
            cursor: default_icon_cursor(),
            selected_prefix: default_icon_selected_prefix(),
            progress_filled: default_icon_progress_filled(),
            progress_empty: default_icon_progress_empty(),
            separator: default_icon_separator(),
        }
    }
}

// ─── Sub-themes ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PathBarTheme {
    #[serde(default = "default_path_label_fg")]
    pub label_fg: ThemeColor,
    #[serde(default = "default_path_segment_fg")]
    pub segment_fg: ThemeColor,
    #[serde(default = "default_path_cursor_fg")]
    pub cursor_fg: ThemeColor,
    #[serde(default = "default_path_cursor_bg")]
    pub cursor_bg: ThemeColor,
    #[serde(default = "default_path_cursor_modifier")]
    pub cursor_modifier: ThemeModifier,
}
fn default_path_label_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }
fn default_path_segment_fg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_path_cursor_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_path_cursor_bg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_path_cursor_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }

impl Default for PathBarTheme {
    fn default() -> Self {
        Self {
            label_fg: default_path_label_fg(),
            segment_fg: default_path_segment_fg(),
            cursor_fg: default_path_cursor_fg(),
            cursor_bg: default_path_cursor_bg(),
            cursor_modifier: default_path_cursor_modifier(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct FileListTheme {
    #[serde(default = "default_file_highlight_bg")]
    pub highlight_bg: ThemeColor,
    #[serde(default = "default_file_highlight_fg")]
    pub highlight_fg: ThemeColor,
    #[serde(default = "default_file_selected_fg")]
    pub selected_fg: ThemeColor,
    #[serde(default = "default_file_normal_fg")]
    pub normal_fg: ThemeColor,
}
fn default_file_highlight_bg() -> ThemeColor { ThemeColor::rgb(42, 54, 79) }
fn default_file_highlight_fg() -> ThemeColor { ThemeColor::rgb(197, 210, 223) }
fn default_file_selected_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_file_normal_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }

impl Default for FileListTheme {
    fn default() -> Self {
        Self {
            highlight_bg: default_file_highlight_bg(),
            highlight_fg: default_file_highlight_fg(),
            selected_fg: default_file_selected_fg(),
            normal_fg: default_file_normal_fg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PreviewTheme {
    #[serde(default = "default_preview_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_preview_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_preview_content_fg")]
    pub content_fg: ThemeColor,
    #[serde(default = "default_preview_content_bg")]
    pub content_bg: ThemeColor,
}
fn default_preview_border_fg() -> ThemeColor { ThemeColor::rgb(115, 218, 202) }
fn default_preview_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_preview_content_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_preview_content_bg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }

impl Default for PreviewTheme {
    fn default() -> Self {
        Self {
            border_fg: default_preview_border_fg(),
            title_fg: default_preview_title_fg(),
            content_fg: default_preview_content_fg(),
            content_bg: default_preview_content_bg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SuggestionsTheme {
    #[serde(default = "default_suggest_item_fg")]
    pub item_fg: ThemeColor,
    #[serde(default = "default_suggest_selected_bg")]
    pub selected_bg: ThemeColor,
}
fn default_suggest_item_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }
fn default_suggest_selected_bg() -> ThemeColor { ThemeColor::rgb(65, 72, 104) }

impl Default for SuggestionsTheme {
    fn default() -> Self {
        Self {
            item_fg: default_suggest_item_fg(),
            selected_bg: default_suggest_selected_bg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct BookmarksTheme {
    #[serde(default = "default_bm_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_bm_border_modifier")]
    pub border_modifier: ThemeModifier,
    #[serde(default = "default_bm_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_bm_title_modifier")]
    pub title_modifier: ThemeModifier,
    #[serde(default = "default_bm_highlight_bg")]
    pub highlight_bg: ThemeColor,
    #[serde(default = "default_bm_highlight_fg")]
    pub highlight_fg: ThemeColor,
    #[serde(default = "default_bm_highlight_modifier")]
    pub highlight_modifier: ThemeModifier,
    #[serde(default = "default_bm_empty_fg")]
    pub empty_fg: ThemeColor,
    #[serde(default = "default_bm_empty_modifier")]
    pub empty_modifier: ThemeModifier,
}
fn default_bm_border_fg() -> ThemeColor { ThemeColor::rgb(187, 154, 247) }
fn default_bm_border_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_bm_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_bm_title_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_bm_highlight_bg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_bm_highlight_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_bm_highlight_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_bm_empty_fg() -> ThemeColor { ThemeColor::rgb(247, 118, 142) }
fn default_bm_empty_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }

impl Default for BookmarksTheme {
    fn default() -> Self {
        Self {
            border_fg: default_bm_border_fg(),
            border_modifier: default_bm_border_modifier(),
            title_fg: default_bm_title_fg(),
            title_modifier: default_bm_title_modifier(),
            highlight_bg: default_bm_highlight_bg(),
            highlight_fg: default_bm_highlight_fg(),
            highlight_modifier: default_bm_highlight_modifier(),
            empty_fg: default_bm_empty_fg(),
            empty_modifier: default_bm_empty_modifier(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct GrepResultsTheme {
    #[serde(default = "default_grep_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_grep_border_modifier")]
    pub border_modifier: ThemeModifier,
    #[serde(default = "default_grep_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_grep_title_modifier")]
    pub title_modifier: ThemeModifier,
    #[serde(default = "default_grep_highlight_bg")]
    pub highlight_bg: ThemeColor,
    #[serde(default = "default_grep_highlight_fg")]
    pub highlight_fg: ThemeColor,
    #[serde(default = "default_grep_highlight_modifier")]
    pub highlight_modifier: ThemeModifier,
}
fn default_grep_border_fg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }
fn default_grep_border_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_grep_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_grep_title_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_grep_highlight_bg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }
fn default_grep_highlight_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_grep_highlight_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }

impl Default for GrepResultsTheme {
    fn default() -> Self {
        Self {
            border_fg: default_grep_border_fg(),
            border_modifier: default_grep_border_modifier(),
            title_fg: default_grep_title_fg(),
            title_modifier: default_grep_title_modifier(),
            highlight_bg: default_grep_highlight_bg(),
            highlight_fg: default_grep_highlight_fg(),
            highlight_modifier: default_grep_highlight_modifier(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ViewerTheme {
    #[serde(default = "default_viewer_line_numbers_fg")]
    pub line_numbers_fg: ThemeColor,
    #[serde(default = "default_viewer_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_viewer_border_fg")]
    pub border_fg: ThemeColor,
}
fn default_viewer_line_numbers_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }
fn default_viewer_title_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_viewer_border_fg() -> ThemeColor { ThemeColor::rgb(65, 72, 104) }

impl Default for ViewerTheme {
    fn default() -> Self {
        Self {
            line_numbers_fg: default_viewer_line_numbers_fg(),
            title_fg: default_viewer_title_fg(),
            border_fg: default_viewer_border_fg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct EditorTheme {
    #[serde(default = "default_editor_line_numbers_fg")]
    pub line_numbers_fg: ThemeColor,
    #[serde(default = "default_editor_selection_fg")]
    pub selection_fg: ThemeColor,
    #[serde(default = "default_editor_selection_bg")]
    pub selection_bg: ThemeColor,
    #[serde(default = "default_editor_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_editor_border_fg")]
    pub border_fg: ThemeColor,
}
fn default_editor_line_numbers_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }
fn default_editor_selection_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_editor_selection_bg() -> ThemeColor { ThemeColor::rgb(65, 72, 104) }
fn default_editor_title_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_editor_border_fg() -> ThemeColor { ThemeColor::rgb(65, 72, 104) }

impl Default for EditorTheme {
    fn default() -> Self {
        Self {
            line_numbers_fg: default_editor_line_numbers_fg(),
            selection_fg: default_editor_selection_fg(),
            selection_bg: default_editor_selection_bg(),
            title_fg: default_editor_title_fg(),
            border_fg: default_editor_border_fg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct DiskUsageTheme {
    #[serde(default = "default_du_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_du_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_du_title_modifier")]
    pub title_modifier: ThemeModifier,
    #[serde(default = "default_du_cursor_bg")]
    pub cursor_bg: ThemeColor,
    #[serde(default = "default_du_cursor_fg")]
    pub cursor_fg: ThemeColor,
    #[serde(default = "default_du_cursor_modifier")]
    pub cursor_modifier: ThemeModifier,
}
fn default_du_border_fg() -> ThemeColor { ThemeColor::rgb(115, 218, 202) }
fn default_du_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_du_title_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_du_cursor_bg() -> ThemeColor { ThemeColor::rgb(42, 54, 79) }
fn default_du_cursor_fg() -> ThemeColor { ThemeColor::rgb(197, 210, 223) }
fn default_du_cursor_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }

impl Default for DiskUsageTheme {
    fn default() -> Self {
        Self {
            border_fg: default_du_border_fg(),
            title_fg: default_du_title_fg(),
            title_modifier: default_du_title_modifier(),
            cursor_bg: default_du_cursor_bg(),
            cursor_fg: default_du_cursor_fg(),
            cursor_modifier: default_du_cursor_modifier(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct HelpTheme {
    #[serde(default = "default_help_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_help_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_help_title_modifier")]
    pub title_modifier: ThemeModifier,
    #[serde(default = "default_help_section_fg")]
    pub section_fg: ThemeColor,
    #[serde(default = "default_help_section_modifier")]
    pub section_modifier: ThemeModifier,
}
fn default_help_border_fg() -> ThemeColor { ThemeColor::rgb(115, 218, 202) }
fn default_help_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_help_title_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_help_section_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_help_section_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }

impl Default for HelpTheme {
    fn default() -> Self {
        Self {
            border_fg: default_help_border_fg(),
            title_fg: default_help_title_fg(),
            title_modifier: default_help_title_modifier(),
            section_fg: default_help_section_fg(),
            section_modifier: default_help_section_modifier(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct FileInfoTheme {
    #[serde(default = "default_fi_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_fi_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_fi_title_modifier")]
    pub title_modifier: ThemeModifier,
    #[serde(default = "default_fi_hint_fg")]
    pub hint_fg: ThemeColor,
}
fn default_fi_border_fg() -> ThemeColor { ThemeColor::rgb(115, 218, 202) }
fn default_fi_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_fi_title_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_fi_hint_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }

impl Default for FileInfoTheme {
    fn default() -> Self {
        Self {
            border_fg: default_fi_border_fg(),
            title_fg: default_fi_title_fg(),
            title_modifier: default_fi_title_modifier(),
            hint_fg: default_fi_hint_fg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PromptTheme {
    #[serde(default = "default_prompt_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_prompt_border_modifier")]
    pub border_modifier: ThemeModifier,
    #[serde(default = "default_prompt_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_prompt_title_modifier")]
    pub title_modifier: ThemeModifier,
    #[serde(default = "default_prompt_hint_fg")]
    pub hint_fg: ThemeColor,
}
fn default_prompt_border_fg() -> ThemeColor { ThemeColor::rgb(115, 218, 202) }
fn default_prompt_border_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_prompt_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_prompt_title_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_prompt_hint_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }

impl Default for PromptTheme {
    fn default() -> Self {
        Self {
            border_fg: default_prompt_border_fg(),
            border_modifier: default_prompt_border_modifier(),
            title_fg: default_prompt_title_fg(),
            title_modifier: default_prompt_title_modifier(),
            hint_fg: default_prompt_hint_fg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct StatusBarTheme {
    #[serde(default = "default_sb_normal_fg")]
    pub normal_fg: ThemeColor,
    #[serde(default = "default_sb_normal_bg")]
    pub normal_bg: ThemeColor,
    #[serde(default = "default_sb_status_msg_fg")]
    pub status_msg_fg: ThemeColor,
    #[serde(default = "default_sb_status_msg_bg")]
    pub status_msg_bg: ThemeColor,
    #[serde(default = "default_sb_status_msg_modifier")]
    pub status_msg_modifier: ThemeModifier,
    #[serde(default = "default_sb_command_fg")]
    pub command_fg: ThemeColor,
    #[serde(default = "default_sb_command_bg")]
    pub command_bg: ThemeColor,
    #[serde(default = "default_sb_command_border_fg")]
    pub command_border_fg: ThemeColor,
    #[serde(default = "default_sb_search_fg")]
    pub search_fg: ThemeColor,
    #[serde(default = "default_sb_search_bg")]
    pub search_bg: ThemeColor,
    #[serde(default = "default_sb_breadcrumbs_fg")]
    pub breadcrumbs_fg: ThemeColor,
    #[serde(default = "default_sb_breadcrumbs_bg")]
    pub breadcrumbs_bg: ThemeColor,
    #[serde(default = "default_sb_bookmarks_fg")]
    pub bookmarks_fg: ThemeColor,
    #[serde(default = "default_sb_bookmarks_bg")]
    pub bookmarks_bg: ThemeColor,
    #[serde(default = "default_sb_grep_results_fg")]
    pub grep_results_fg: ThemeColor,
    #[serde(default = "default_sb_grep_results_bg")]
    pub grep_results_bg: ThemeColor,
    #[serde(default = "default_sb_disk_usage_fg")]
    pub disk_usage_fg: ThemeColor,
    #[serde(default = "default_sb_disk_usage_bg")]
    pub disk_usage_bg: ThemeColor,
    #[serde(default = "default_sb_confirm_delete_fg")]
    pub confirm_delete_fg: ThemeColor,
    #[serde(default = "default_sb_confirm_delete_bg")]
    pub confirm_delete_bg: ThemeColor,
    #[serde(default = "default_sb_confirm_delete_modifier")]
    pub confirm_delete_modifier: ThemeModifier,
    #[serde(default = "default_sb_viewer_fg")]
    pub viewer_fg: ThemeColor,
    #[serde(default = "default_sb_viewer_bg")]
    pub viewer_bg: ThemeColor,
    #[serde(default = "default_sb_editor_fg")]
    pub editor_fg: ThemeColor,
    #[serde(default = "default_sb_editor_bg")]
    pub editor_bg: ThemeColor,
    #[serde(default = "default_sb_file_info_fg")]
    pub file_info_fg: ThemeColor,
    #[serde(default = "default_sb_file_info_bg")]
    pub file_info_bg: ThemeColor,
    #[serde(default = "default_sb_help_fg")]
    pub help_fg: ThemeColor,
    #[serde(default = "default_sb_help_bg")]
    pub help_bg: ThemeColor,
    #[serde(default = "default_sb_action_fg")]
    pub action_fg: ThemeColor,
    #[serde(default = "default_sb_action_bg")]
    pub action_bg: ThemeColor,
    #[serde(default = "default_sb_open_with_fg")]
    pub open_with_fg: ThemeColor,
    #[serde(default = "default_sb_open_with_bg")]
    pub open_with_bg: ThemeColor,
    #[serde(default = "default_sb_save_fg")]
    pub save_fg: ThemeColor,
    #[serde(default = "default_sb_save_bg")]
    pub save_bg: ThemeColor,
    #[serde(default = "default_sb_save_modifier")]
    pub save_modifier: ThemeModifier,
}
fn default_sb_normal_fg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }
fn default_sb_normal_bg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_status_msg_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_sb_status_msg_bg() -> ThemeColor { ThemeColor::rgb(247, 118, 142) }
fn default_sb_status_msg_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_sb_command_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_sb_command_bg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_command_border_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_sb_search_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_search_bg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_sb_breadcrumbs_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_breadcrumbs_bg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_sb_bookmarks_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_bookmarks_bg() -> ThemeColor { ThemeColor::rgb(187, 154, 247) }
fn default_sb_grep_results_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_sb_grep_results_bg() -> ThemeColor { ThemeColor::rgb(65, 72, 104) }
fn default_sb_disk_usage_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_disk_usage_bg() -> ThemeColor { ThemeColor::rgb(115, 218, 202) }
fn default_sb_confirm_delete_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_sb_confirm_delete_bg() -> ThemeColor { ThemeColor::rgb(247, 118, 142) }
fn default_sb_confirm_delete_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_sb_viewer_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_viewer_bg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_sb_editor_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_editor_bg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_sb_file_info_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_sb_file_info_bg() -> ThemeColor { ThemeColor::rgb(65, 72, 104) }
fn default_sb_help_fg() -> ThemeColor { ThemeColor::rgb(205, 210, 217) }
fn default_sb_help_bg() -> ThemeColor { ThemeColor::rgb(65, 72, 104) }
fn default_sb_save_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_save_bg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }
fn default_sb_save_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_sb_action_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_action_bg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_sb_open_with_fg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_sb_open_with_bg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }

impl Default for StatusBarTheme {
    fn default() -> Self {
        Self {
            normal_fg: default_sb_normal_fg(),
            normal_bg: default_sb_normal_bg(),
            status_msg_fg: default_sb_status_msg_fg(),
            status_msg_bg: default_sb_status_msg_bg(),
            status_msg_modifier: default_sb_status_msg_modifier(),
            command_fg: default_sb_command_fg(),
            command_bg: default_sb_command_bg(),
            command_border_fg: default_sb_command_border_fg(),
            search_fg: default_sb_search_fg(),
            search_bg: default_sb_search_bg(),
            breadcrumbs_fg: default_sb_breadcrumbs_fg(),
            breadcrumbs_bg: default_sb_breadcrumbs_bg(),
            bookmarks_fg: default_sb_bookmarks_fg(),
            bookmarks_bg: default_sb_bookmarks_bg(),
            grep_results_fg: default_sb_grep_results_fg(),
            grep_results_bg: default_sb_grep_results_bg(),
            disk_usage_fg: default_sb_disk_usage_fg(),
            disk_usage_bg: default_sb_disk_usage_bg(),
            confirm_delete_fg: default_sb_confirm_delete_fg(),
            confirm_delete_bg: default_sb_confirm_delete_bg(),
            confirm_delete_modifier: default_sb_confirm_delete_modifier(),
            viewer_fg: default_sb_viewer_fg(),
            viewer_bg: default_sb_viewer_bg(),
            editor_fg: default_sb_editor_fg(),
            editor_bg: default_sb_editor_bg(),
            file_info_fg: default_sb_file_info_fg(),
            file_info_bg: default_sb_file_info_bg(),
            help_fg: default_sb_help_fg(),
            help_bg: default_sb_help_bg(),
            save_fg: default_sb_save_fg(),
            save_bg: default_sb_save_bg(),
            save_modifier: default_sb_save_modifier(),
            action_fg: default_sb_action_fg(),
            action_bg: default_sb_action_bg(),
            open_with_fg: default_sb_open_with_fg(),
            open_with_bg: default_sb_open_with_bg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct MarkdownTheme {
    #[serde(default = "default_md_heading_fg")]
    pub heading_fg: ThemeColor,
    #[serde(default = "default_md_heading_modifier")]
    pub heading_modifier: ThemeModifier,
    #[serde(default = "default_md_italic_modifier")]
    pub italic_modifier: ThemeModifier,
    #[serde(default = "default_md_bold_modifier")]
    pub bold_modifier: ThemeModifier,
    #[serde(default = "default_md_code_fg")]
    pub code_fg: ThemeColor,
    #[serde(default = "default_md_code_bg")]
    pub code_bg: ThemeColor,
    #[serde(default = "default_md_inline_code_fg")]
    pub inline_code_fg: ThemeColor,
    #[serde(default = "default_md_link_fg")]
    pub link_fg: ThemeColor,
    #[serde(default = "default_md_link_modifier")]
    pub link_modifier: ThemeModifier,
    #[serde(default = "default_md_blockquote_fg")]
    pub blockquote_fg: ThemeColor,
    #[serde(default = "default_md_blockquote_modifier")]
    pub blockquote_modifier: ThemeModifier,
    #[serde(default = "default_md_hr_fg")]
    pub hr_fg: ThemeColor,
}
fn default_md_heading_fg() -> ThemeColor { ThemeColor::rgb(197, 210, 223) }
fn default_md_heading_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_md_italic_modifier() -> ThemeModifier { ThemeModifier(Modifier::ITALIC) }
fn default_md_bold_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_md_code_fg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }
fn default_md_code_bg() -> ThemeColor { ThemeColor::rgb(26, 27, 30) }
fn default_md_inline_code_fg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }
fn default_md_link_fg() -> ThemeColor { ThemeColor::rgb(122, 162, 247) }
fn default_md_link_modifier() -> ThemeModifier { ThemeModifier(Modifier::UNDERLINED) }
fn default_md_blockquote_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }
fn default_md_blockquote_modifier() -> ThemeModifier { ThemeModifier(Modifier::ITALIC) }
fn default_md_hr_fg() -> ThemeColor { ThemeColor::rgb(86, 95, 137) }

impl Default for MarkdownTheme {
    fn default() -> Self {
        Self {
            heading_fg: default_md_heading_fg(),
            heading_modifier: default_md_heading_modifier(),
            italic_modifier: default_md_italic_modifier(),
            bold_modifier: default_md_bold_modifier(),
            code_fg: default_md_code_fg(),
            code_bg: default_md_code_bg(),
            inline_code_fg: default_md_inline_code_fg(),
            link_fg: default_md_link_fg(),
            link_modifier: default_md_link_modifier(),
            blockquote_fg: default_md_blockquote_fg(),
            blockquote_modifier: default_md_blockquote_modifier(),
            hr_fg: default_md_hr_fg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct GrepHintTheme {
    #[serde(default = "default_gh_valid_fg")]
    pub valid_fg: ThemeColor,
    #[serde(default = "default_gh_invalid_fg")]
    pub invalid_fg: ThemeColor,
}
fn default_gh_valid_fg() -> ThemeColor { ThemeColor::rgb(158, 206, 106) }
fn default_gh_invalid_fg() -> ThemeColor { ThemeColor::rgb(247, 118, 142) }

impl Default for GrepHintTheme {
    fn default() -> Self {
        Self {
            valid_fg: default_gh_valid_fg(),
            invalid_fg: default_gh_invalid_fg(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ActionMenuTheme {
    #[serde(default = "default_am_border_fg")]
    pub border_fg: ThemeColor,
    #[serde(default = "default_am_title_fg")]
    pub title_fg: ThemeColor,
    #[serde(default = "default_am_title_modifier")]
    pub title_modifier: ThemeModifier,
    #[serde(default = "default_am_highlight_bg")]
    pub highlight_bg: ThemeColor,
    #[serde(default = "default_am_highlight_fg")]
    pub highlight_fg: ThemeColor,
}
fn default_am_border_fg() -> ThemeColor { ThemeColor::rgb(115, 218, 202) }
fn default_am_title_fg() -> ThemeColor { ThemeColor::rgb(224, 175, 104) }
fn default_am_title_modifier() -> ThemeModifier { ThemeModifier(Modifier::BOLD) }
fn default_am_highlight_bg() -> ThemeColor { ThemeColor::rgb(42, 54, 79) }
fn default_am_highlight_fg() -> ThemeColor { ThemeColor::rgb(197, 210, 223) }

impl Default for ActionMenuTheme {
    fn default() -> Self {
        Self {
            border_fg: default_am_border_fg(),
            title_fg: default_am_title_fg(),
            title_modifier: default_am_title_modifier(),
            highlight_bg: default_am_highlight_bg(),
            highlight_fg: default_am_highlight_fg(),
        }
    }
}

// ─── Top-level Theme ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    #[serde(default)]
    pub path_bar: PathBarTheme,
    #[serde(default)]
    pub file_list: FileListTheme,
    #[serde(default)]
    pub preview: PreviewTheme,
    #[serde(default)]
    pub suggestions: SuggestionsTheme,
    #[serde(default)]
    pub bookmarks: BookmarksTheme,
    #[serde(default)]
    pub grep_results: GrepResultsTheme,
    #[serde(default)]
    pub viewer: ViewerTheme,
    #[serde(default)]
    pub editor: EditorTheme,
    #[serde(default)]
    pub disk_usage: DiskUsageTheme,
    #[serde(default)]
    pub help: HelpTheme,
    #[serde(default)]
    pub file_info: FileInfoTheme,
    #[serde(default)]
    pub prompt: PromptTheme,
    #[serde(default)]
    pub status_bar: StatusBarTheme,
    #[serde(default)]
    pub markdown: MarkdownTheme,
    #[serde(default)]
    pub grep_hint: GrepHintTheme,
    #[serde(default)]
    pub action_menu: ActionMenuTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            path_bar: PathBarTheme::default(),
            file_list: FileListTheme::default(),
            preview: PreviewTheme::default(),
            suggestions: SuggestionsTheme::default(),
            bookmarks: BookmarksTheme::default(),
            grep_results: GrepResultsTheme::default(),
            viewer: ViewerTheme::default(),
            editor: EditorTheme::default(),
            disk_usage: DiskUsageTheme::default(),
            help: HelpTheme::default(),
            file_info: FileInfoTheme::default(),
            prompt: PromptTheme::default(),
            status_bar: StatusBarTheme::default(),
            markdown: MarkdownTheme::default(),
            grep_hint: GrepHintTheme::default(),
            action_menu: ActionMenuTheme::default(),
        }
    }
}

// ─── AppearanceConfig (wraps theme + icons) ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AppearanceConfig {
    #[serde(default)]
    pub icons: Icons,
    #[serde(default)]
    pub theme: Theme,
}

impl AppearanceConfig {
    pub fn load() -> Self {
        let candidates = [
            dirs::config_dir().map(|d| d.join("texp").join("config.toml")),
            Some(std::path::PathBuf::from("texp.toml")),
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

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            icons: Icons::default(),
            theme: Theme::default(),
        }
    }
}
