use crate::app::{App, AppMode, editor_line_starts};
use crate::state::*;
use crate::theme::Icons;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::fs;
use std::time::Duration;

pub fn draw(f: &mut Frame, app: &mut App) {
    let theme = &app.config.appearance.theme;
    let icons = &app.config.appearance.icons;
    let snippet_height = if app.mode == AppMode::Command && !app.cmd.command_suggestion.is_empty() {
        let max_height = f.area().height.saturating_sub(6).clamp(3, 25);
        std::cmp::min(app.cmd.command_suggestion.len() as u16 + 2, max_height)
    } else {
        0
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(snippet_height),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.area());

    // ── Path bar ────────────────────────────────────────────────
    let mut path_spans = Vec::new();
    path_spans.push(Span::styled(
        " PATH: ",
        Style::default().fg(theme.path_bar.label_fg.0),
    ));
    for (idx, segment) in app.nav.path_segments.iter().enumerate() {
        let name = match segment.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => segment.to_string_lossy().to_string(),
        };
        let mut style = Style::default().fg(theme.path_bar.segment_fg.0);
        if app.mode == AppMode::Breadcrumbs && idx == app.nav.path_cursor {
            style = style
                .fg(theme.path_bar.cursor_fg.0)
                .bg(theme.path_bar.cursor_bg.0)
                .add_modifier(theme.path_bar.cursor_modifier.0);
        }
        path_spans.push(Span::styled(name, style));
        if idx < app.nav.path_segments.len() - 1 {
            path_spans.push(Span::raw(&icons.separator));
        }
    }
    f.render_widget(Paragraph::new(Line::from(path_spans)), chunks[0]);

    // ── File list ───────────────────────────────────────────────
    let viewport = chunks[2].height.saturating_sub(2) as usize;
    let total = app.nav.files.len();
    let half = viewport / 2;
    let start = if total <= viewport {
        0
    } else {
        app.nav
            .cursor_index
            .saturating_sub(half)
            .min(total.saturating_sub(viewport))
    };
    let end = std::cmp::min(start + viewport, total);

    let is_search = app.mode == AppMode::Search;
    let mut items = Vec::new();
    for i in start..end {
        let path = &app.nav.files[i];
        let display_name = if is_search {
            path.strip_prefix(&app.nav.current_dir)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string()
        } else {
            match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => path.to_string_lossy().to_string(),
            }
        };
        let is_dir = app
            .nav
            .dir_cache
            .get(path)
            .map(|e| e.is_dir)
            .unwrap_or_else(|| path.is_dir());
        let prefix = if is_dir {
            &icons.folder
        } else {
            &icons.file
        };
        let bookmark_sign = if app.bookmarks.bookmarks.contains(path) {
            format!("{} ", icons.bookmarked)
        } else {
            String::new()
        };

        let mut display_text = format!("{}{}{}", bookmark_sign, prefix, display_name);
        let is_selected = app.nav.selected_files.contains(path);
        if is_selected {
            display_text = format!("{}{}", icons.selected_prefix, display_text);
        }
        let mut style = Style::default().fg(theme.file_list.normal_fg.0);
        if is_selected {
            style = style.fg(theme.file_list.selected_fg.0);
        }
        items.push(ListItem::new(display_text).style(style));
    }
    app.nav
        .list_state
        .select(Some(app.nav.cursor_index.saturating_sub(start)));
    let list_title = if is_search { " Search Results " } else { " Files " };
    let mut list = List::new(items).block(Block::default().borders(Borders::ALL).title(list_title));
    if app.mode == AppMode::Normal || app.mode == AppMode::Search {
        list = list.highlight_style(Style::default().bg(theme.file_list.highlight_bg.0).fg(theme.file_list.highlight_fg.0));
    }

    if app.mode == AppMode::Normal && app.preview.preview_visible {
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(chunks[2]);
        f.render_stateful_widget(list, main_layout[0], &mut app.nav.list_state);
        let preview_style = Style::default().fg(theme.preview.content_fg.0).bg(theme.preview.content_bg.0);
        let preview: Paragraph =
            if app.preview.preview_is_md && !app.preview.preview_lines.is_empty() {
                Paragraph::new(app.preview.preview_lines.clone()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.preview.border_fg.0))
                        .title(Span::styled("Preview ", Style::default().fg(theme.preview.title_fg.0))),
                )
            } else {
                Paragraph::new(app.preview.preview_content.as_str())
                    .style(preview_style)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.preview.border_fg.0))
                            .title(Span::styled("Preview ", Style::default().fg(theme.preview.title_fg.0))),
                    )
            };
        f.render_widget(preview, main_layout[1]);
    } else {
        f.render_stateful_widget(list, chunks[2], &mut app.nav.list_state);
    }

    // ── Suggestions ─────────────────────────────────────────────
    if snippet_height > 0 {
        let max_visible = snippet_height.saturating_sub(2) as usize;
        let total = app.cmd.command_suggestion.len();
        if app.cmd.suggestion_index >= app.cmd.suggestion_scroll + max_visible {
            app.cmd.suggestion_scroll = app.cmd.suggestion_index - max_visible + 1;
        }
        if app.cmd.suggestion_index < app.cmd.suggestion_scroll {
            app.cmd.suggestion_scroll = app.cmd.suggestion_index;
        }
        let end = std::cmp::min(app.cmd.suggestion_scroll + max_visible, total);
        let mut suggest_items = Vec::new();
        if app.cmd.suggestion_scroll > 0 {
            suggest_items.push(ListItem::new("  ↑ more ↑"));
        }
        for abs_idx in app.cmd.suggestion_scroll..end {
            let mut style = Style::default().fg(theme.suggestions.item_fg.0);
            let prefix = if abs_idx == app.cmd.suggestion_index {
                style = style.bg(theme.suggestions.selected_bg.0);
                "> "
            } else {
                "  "
            };
            suggest_items.push(
                ListItem::new(format!(
                    "{}{}",
                    prefix, &app.cmd.command_suggestion[abs_idx]
                ))
                .style(style),
            );
        }
        if end < total {
            suggest_items.push(ListItem::new(format!(
                "  ↓ more ({}/{}) ↓",
                app.cmd.suggestion_index + 1,
                total
            )));
        }
        let title = if total > max_visible {
            format!(
                " Suggestions ({}/{}) ",
                app.cmd.suggestion_index + 1,
                total
            )
        } else {
            " Suggestions ".to_string()
        };
        let suggestion_block = List::new(suggest_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        );
        f.render_widget(suggestion_block, chunks[1]);
    }
    // ── Bookmarks ───────────────────────────────────────────────
    if app.mode == AppMode::BookMarks {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - 60) / 2),
                Constraint::Percentage(60),
                Constraint::Percentage((100 - 60) / 2),
            ])
            .split(f.area());
        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - 70) / 2),
                Constraint::Percentage(70),
                Constraint::Percentage((100 - 70) / 2),
            ])
            .split(popup_layout[1])[1];
        f.render_widget(Clear, area);

        let mut bookmark_items = Vec::new();
        if app.bookmarks.bookmarks.is_empty() {
            bookmark_items.push(ListItem::new(""));
            bookmark_items.push(
                ListItem::new(" Bookmark list is empty!")
                    .style(Style::default().fg(theme.bookmarks.empty_fg.0).add_modifier(theme.bookmarks.empty_modifier.0)),
            );
            bookmark_items.push(ListItem::new(""));
            bookmark_items.push(ListItem::new("  How to add a bookmark:"));
            bookmark_items.push(ListItem::new("  1. Press [Esc] to close this window."));
            bookmark_items.push(ListItem::new("  2. Select an item in the file list."));
            bookmark_items.push(ListItem::new(
                format!("  3. Press [b] to toggle bookmark ({} appears).", icons.bookmarked),
            ));
            bookmark_items.push(ListItem::new("  4. Open bookmarks again via [Shift + B]."));
        } else {
            for (idx, path) in app.bookmarks.bookmarks.iter().enumerate() {
                let prefix = if idx == app.bookmarks.bookmark_cursor {
                    format!("{}{} ", icons.cursor, icons.bookmarked)
                } else {
                    format!("   {} ", icons.bookmarked)
                };
                bookmark_items.push(ListItem::new(format!("{}{}", prefix, path.display())));
            }
        }

        let bookmark_block = Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(theme.bookmarks.border_fg.0)
                    .add_modifier(theme.bookmarks.border_modifier.0),
            )
            .title(Span::styled(
                " BOOKMARKS ",
                Style::default()
                    .fg(theme.bookmarks.title_fg.0)
                    .add_modifier(theme.bookmarks.title_modifier.0),
            ));
        let bookmark_list = List::new(bookmark_items)
            .block(bookmark_block)
            .highlight_style(
                Style::default()
                    .bg(theme.bookmarks.highlight_bg.0)
                    .fg(theme.bookmarks.highlight_fg.0)
                    .add_modifier(theme.bookmarks.highlight_modifier.0),
            );
        f.render_stateful_widget(bookmark_list, area, &mut app.bookmarks.bookmarks_state);
    }
    if app.mode == AppMode::GrepResults {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(10),
                Constraint::Percentage(80),
                Constraint::Percentage(10),
            ])
            .split(f.area());
        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(5),
                Constraint::Percentage(90),
                Constraint::Percentage(5),
            ])
            .split(popup_layout[1])[1];
        f.render_widget(Clear, area);

        let mut grep_items = Vec::new();
        if app.grep.grep_matches.is_empty() {
            grep_items.push(ListItem::new("No matches found."));
        } else {
            for (idx, m) in app.grep.grep_matches.iter().enumerate() {
                let prefix = if idx == app.grep.grep_cursor {
                    "👉 "
                } else {
                    "   "
                };
                let short_name = m
                    .file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("File");
                grep_items.push(ListItem::new(format!(
                    "{} Line {}:{} [{}] > \"{}\"",
                    prefix,
                    m.line,
                    m.word,
                    short_name,
                    m.text.trim()
                )));
            }
        }

        let grep_block = Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(theme.grep_results.border_fg.0)
                    .add_modifier(theme.grep_results.border_modifier.0),
            )
            .title(Span::styled(
                " GREP RESULTS ",
                Style::default()
                    .fg(theme.grep_results.title_fg.0)
                    .add_modifier(theme.grep_results.title_modifier.0),
            ));
        let grep_list = List::new(grep_items).block(grep_block).highlight_style(
            Style::default()
                .bg(theme.grep_results.highlight_bg.0)
                .fg(theme.grep_results.highlight_fg.0)
                .add_modifier(theme.grep_results.highlight_modifier.0),
        );
        f.render_stateful_widget(grep_list, area, &mut app.grep.grep_state);
    }
    if app.mode == AppMode::Viewer {
        let area = chunks[2];
        let file_name = app
            .nav
            .files
            .get(app.nav.cursor_index)
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("File");
        let visible_height = area.height.saturating_sub(2) as usize;
        let total_lines = app.preview.preview_content.lines().count();
        if total_lines > visible_height && app.preview.preview_scroll + visible_height > total_lines
        {
            app.preview.preview_scroll = total_lines - visible_height;
        }
        let num_style = Style::default().fg(theme.viewer.line_numbers_fg.0);
        let text: Vec<Line> = if app.preview.preview_is_md && !app.preview.preview_lines.is_empty()
        {
            app.preview
                .preview_lines
                .iter()
                .enumerate()
                .skip(app.preview.preview_scroll)
                .take(visible_height)
                .map(|(i, l)| {
                    let mut spans = vec![Span::styled(format!("{:>4}| ", i + 1), num_style)];
                    spans.extend(l.clone().spans);
                    Line::from(spans)
                })
                .collect()
        } else {
            app.preview
                .preview_content
                .lines()
                .enumerate()
                .skip(app.preview.preview_scroll)
                .take(visible_height)
                .map(|(i, l)| {
                    Line::from(vec![
                        Span::styled(format!("{:>4}| ", i + 1), num_style),
                        Span::raw(l.to_string()),
                    ])
                })
                .collect()
        };
        let scroll_info = if total_lines > visible_height {
            let pct = (app.preview.preview_scroll as f64 / (total_lines - visible_height) as f64
                * 100.0) as u16;
            format!(" View: {} [{}%] ", file_name, pct.min(100))
        } else {
            format!(" View: {} ", file_name)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.viewer.border_fg.0))
            .title(Span::styled(scroll_info, Style::default().fg(theme.viewer.title_fg.0)));
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
    }
    if app.mode == AppMode::Editor {
        let area = chunks[2];
        let visible_height = area.height.saturating_sub(2) as usize;
        let starts = editor_line_starts(&app.editor.editor_buffer);
        let total_lines = starts.len();
        if app.editor.editor_cursor_line < app.editor.editor_scroll {
            app.editor.editor_scroll = app.editor.editor_cursor_line;
        }
        if app.editor.editor_scroll + visible_height > total_lines && total_lines > visible_height {
            app.editor.editor_scroll = total_lines - visible_height;
        }
        if app.editor.editor_cursor_line >= app.editor.editor_scroll + visible_height {
            app.editor.editor_scroll = app.editor.editor_cursor_line - visible_height + 1;
        }
        let sel = app.editor.editor_selection;
        let cursor = (app.editor.editor_cursor_line, app.editor.editor_cursor_col);
        let (sel_start, sel_end) = match sel {
            Some(anchor) => {
                let a = (anchor.0, anchor.1);
                if a < cursor { (a, cursor) } else { (cursor, a) }
            }
            None => ((0, 0), (0, 0)),
        };
        let start_idx = app.editor.editor_scroll.min(starts.len().saturating_sub(1));
        let end_idx = (start_idx + visible_height).min(starts.len());
        let mut text: Vec<Line> = Vec::new();
        for i in start_idx..end_idx {
            let line_start = starts[i];
            let line_end = app.editor.editor_buffer[line_start..]
                .find('\n')
                .map(|pos| line_start + pos)
                .unwrap_or(app.editor.editor_buffer.len());
            let content = &app.editor.editor_buffer[line_start..line_end];
            let line_len = content.chars().count();
            let num = format!("{:>4}| ", i + 1);
            let num_style = Style::default().fg(theme.editor.line_numbers_fg.0);
            if sel.is_some() && i >= sel_start.0 && i <= sel_end.0 {
                let left = if i == sel_start.0 {
                    sel_start.1.min(line_len)
                } else {
                    0
                };
                let right = if i == sel_end.0 {
                    sel_end.1.min(line_len)
                } else {
                    line_len
                };
                let before = content.chars().take(left).collect::<String>();
                let sel_text = content
                    .chars()
                    .skip(left)
                    .take(right.saturating_sub(left))
                    .collect::<String>();
                let after = content.chars().skip(right).collect::<String>();
                text.push(Line::from(vec![
                    Span::styled(num, num_style),
                    Span::raw(before),
                    Span::styled(sel_text, Style::default().fg(theme.editor.selection_fg.0).bg(theme.editor.selection_bg.0)),
                    Span::raw(after),
                ]));
            } else {
                text.push(Line::from(vec![
                    Span::styled(num, num_style),
                    Span::raw(content),
                ]));
            }
        }
        let file_name = app
            .editor
            .editor_file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("File");
        let title = format!(
            " Editor: {} {} | Ln {} Col {} ",
            file_name,
            if app.editor.editor_modified {
                "[Modified]"
            } else {
                ""
            },
            app.editor.editor_cursor_line + 1,
            app.editor.editor_cursor_col + 1,
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.editor.border_fg.0))
            .title(Span::styled(title, Style::default().fg(theme.editor.title_fg.0)));
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(Clear, area);
        f.render_widget(paragraph, area);
        let cursor_line_vis = app
            .editor
            .editor_cursor_line
            .saturating_sub(app.editor.editor_scroll);
        let cursor_x = area.x + 1 + 6 + app.editor.editor_cursor_col as u16;
        let cursor_y = area.y + 1 + cursor_line_vis as u16;
        if cursor_y < area.y + area.height {
            f.set_cursor_position(Position::new(cursor_x, cursor_y));
        }
    }
    if app.mode == AppMode::DiskUsage {
        let area = chunks[2];
        f.render_widget(Clear, area);
        let mut list_items = Vec::new();
        if app.du.disk_usage_items.is_empty() {
            list_items.push(ListItem::new(" Folder is empty"));
        } else {
            for (idx, item) in app.du.disk_usage_items.iter().enumerate() {
                let prefix = if item.is_dir { &icons.folder } else { &icons.file };
                let cursor = if idx == app.du.disk_usage_cursor {
                    &icons.cursor
                } else {
                    "   "
                };
                let percent = if app.du.disk_usage_total > 0 {
                    (item.size as f32 / app.du.disk_usage_total as f32) * 100.0
                } else {
                    0.0
                };
                let line_text = format!(
                    "{}{} {:<25} {:>10} {}",
                    cursor,
                    prefix,
                    if item.name.len() > 25 {
                        format!("{}...", &item.name[..22])
                    } else {
                        item.name.clone()
                    },
                    format_size(item.size),
                    make_process(percent, icons),
                );
                let mut style = Style::default();
                if idx == app.du.disk_usage_cursor {
                    style = style
                        .bg(theme.disk_usage.cursor_bg.0)
                        .fg(theme.disk_usage.cursor_fg.0)
                        .add_modifier(theme.disk_usage.cursor_modifier.0);
                }
                list_items.push(ListItem::new(line_text).style(style));
            }
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.disk_usage.border_fg.0))
            .title(Span::styled(
                " DISK USAGE (DU) ",
                Style::default()
                    .fg(theme.disk_usage.title_fg.0)
                    .add_modifier(theme.disk_usage.title_modifier.0),
            ));
        f.render_widget(List::new(list_items).block(block), area);
    }
    if app.mode == AppMode::Help {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(5),
                Constraint::Percentage(90),
                Constraint::Percentage(5),
            ])
            .split(f.area());
        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(10),
                Constraint::Percentage(80),
                Constraint::Percentage(10),
            ])
            .split(popup_layout[1])[1];
        f.render_widget(Clear, area);
        let help_lines: Vec<Line> = vec![
            Line::from(Span::styled(
                " KEYBINDINGS ",
                Style::default()
                    .fg(theme.help.section_fg.0)
                    .add_modifier(theme.help.section_modifier.0),
            )),
            Line::from(""),
            Line::from(" ↑/↓           Navigate files"),
            Line::from(" Enter          Open directory / file"),
            Line::from(" Backspace      Parent directory / delete filter char"),
            Line::from(" Esc            Clear filter / back to normal"),
            Line::from(" Space          Toggle selection"),
            Line::from(" :              Command mode"),
            Line::from(" b              Toggle bookmark"),
            Line::from(" B              Bookmarks list"),
            Line::from(" s              Cycle sort (name/date/size/type)"),
            Line::from(" S              Toggle sort order"),
            Line::from(" p              Toggle preview panel"),
            Line::from(" v              Open file viewer"),
            Line::from(" e/i            Open editor (in viewer mode)"),
            Line::from(" .              Toggle hidden files"),
            Line::from(" F1 / ?         This help"),
            Line::from(" Ctrl+A         Action menu (Terminal/Explorer/Default app)"),
            Line::from(" Ctrl+Y         File properties"),
            Line::from(" Ctrl+C         Copy path to clipboard (in properties)"),
            Line::from(" Alt+Left/Right History back/forward"),
            Line::from(" Type chars     Quick filter file list"),
            Line::from(""),
            Line::from(Span::styled(
                " EDITOR CONTROLS",
                Style::default()
                    .fg(theme.help.section_fg.0)
                    .add_modifier(theme.help.section_modifier.0),
            )),
            Line::from(""),
            Line::from(" Ctrl+S         Save"),
            Line::from(" Ctrl+X         Cut selection"),
            Line::from(" Ctrl+C         Copy selection"),
            Line::from(" Ctrl+←/→       Word left/right"),
            Line::from(" Ctrl+Shift+←/→ Select word"),
            Line::from(" Esc            Auto-save and exit"),
            Line::from(""),
            Line::from(Span::styled(
                " COMMANDS (:)",
                Style::default()
                    .fg(theme.help.section_fg.0)
                    .add_modifier(theme.help.section_modifier.0),
            )),
            Line::from(""),
            Line::from(" :cd <path>     Change directory"),
            Line::from(" :rn <name>     Rename file"),
            Line::from(" :cp <dest>     Copy selected"),
            Line::from(" :mv <dest>     Move selected"),
            Line::from(" :rm            Send to Recycle Bin"),
            Line::from(" :mkdir <name>  Create directory"),
            Line::from(" :touch <name>  Create file"),
            Line::from(" :find <name>   Search indexed files by name"),
            Line::from(" :grep <pat>    Search file contents"),
            Line::from(" :du            Analyze disk usage"),
            Line::from(" :index         Rebuild file index"),
            Line::from(" :q             Quit"),
        ];
        let visible_height = area.height.saturating_sub(2) as usize;
        let total = help_lines.len();
        if app.help_scroll + visible_height > total && total > visible_height {
            app.help_scroll = total - visible_height;
        }
        let end = std::cmp::min(app.help_scroll + visible_height, total);
        let visible: Vec<Line> = help_lines[app.help_scroll..end].to_vec();
        let title = if total > visible_height {
            format!(
                " HELP ({}/{}) ",
                (app.help_scroll + visible_height).min(total),
                total
            )
        } else {
            " HELP ".to_string()
        };
        let help_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.help.border_fg.0))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(theme.help.title_fg.0)
                    .add_modifier(theme.help.title_modifier.0),
            ));
        f.render_widget(Paragraph::new(visible).block(help_block), area);
    }
    if app.mode == AppMode::Action {
        let popup = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(f.area())[1];
        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(popup)[1];
        f.render_widget(Clear, area);

        let max_visible = (area.height as usize).saturating_sub(2);
        if app.action.offset > app.action.cursor {
            app.action.offset = app.action.cursor;
        }
        if app.action.cursor >= app.action.offset + max_visible {
            app.action.offset = app.action.cursor.saturating_sub(max_visible).saturating_add(1);
        }

        let visible: Vec<ListItem> = app.action.actions[app.action.offset..]
            .iter()
            .take(max_visible)
            .map(|entry| {
                ListItem::new(format!(" {}\n      {}", entry.label, entry.description))
            })
            .collect();

        let total = app.action.actions.len();
        let action_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.action_menu.border_fg.0))
            .title(Span::styled(
                format!(" ACTION MENU  [{}-{}/{}] ",
                    1 + app.action.offset,
                    (app.action.offset + max_visible).min(total),
                    total),
                Style::default()
                    .fg(theme.action_menu.title_fg.0)
                    .add_modifier(theme.action_menu.title_modifier.0),
            ));
        let action_list = List::new(visible)
            .block(action_block)
            .highlight_symbol(" > ")
            .highlight_style(
            Style::default()
                .bg(theme.action_menu.highlight_bg.0)
                .fg(theme.action_menu.highlight_fg.0),
        );
        f.render_stateful_widget(action_list, area, &mut app.action.list_state);
    }
    if app.mode == AppMode::FileInfo {
        let popup = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(f.area())[1];
        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(popup)[1];
        f.render_widget(Clear, area);
        let path = match app.nav.files.get(app.nav.cursor_index) {
            Some(p) => p,
            None => {
                app.mode = AppMode::Normal;
                return;
            }
        };
        let meta = path.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let size_str = if is_dir {
            let count = fs::read_dir(path).map(|e| e.count()).unwrap_or(0);
            format!("{} items ({})", count, format_size(size))
        } else {
            format_size(size)
        };
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                let secs = dur.as_secs();
                let days = secs / 86400;
                let years = 1970 + (days as f64 / 365.25) as u64;
                let remaining = days - (years - 1970) * 365 - ((years - 1969) / 4);
                let months = [
                    31,
                    if years % 4 == 0 && (years % 100 != 0 || years % 400 == 0) {
                        29
                    } else {
                        28
                    },
                    31,
                    30,
                    31,
                    30,
                    31,
                    31,
                    30,
                    31,
                    30,
                    31,
                ];
                let mut m = 0;
                let mut d = remaining;
                while m < 12 && d >= months[m] {
                    d -= months[m];
                    m += 1;
                }
                format!("{:04}-{:02}-{:02}", years, m + 1, d + 1)
            })
            .unwrap_or_else(|| "unknown".to_string());
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(root)");
        let kind = if is_dir { "Directory" } else { "File" };
        let full = path.to_string_lossy();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("-");
        let info_lines = vec![
            Line::from(Span::styled(
                " FILE PROPERTIES ",
                Style::default()
                    .fg(theme.file_info.title_fg.0)
                    .add_modifier(theme.file_info.title_modifier.0),
            )),
            Line::from(""),
            Line::from(format!(" Name:     {}", name)),
            Line::from(format!(" Path:     {}", full)),
            Line::from(format!(" Type:     {} ({})", kind, ext)),
            Line::from(format!(" Size:     {} ({})", size_str, format_size(size))),
            Line::from(format!(" Modified: {}", modified)),
            Line::from(""),
            Line::from(Span::styled(
                " [Esc/q] Close  [Ctrl+C] Copy path ",
                Style::default().fg(theme.file_info.hint_fg.0),
            )),
        ];
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.file_info.border_fg.0));
        f.render_widget(Paragraph::new(info_lines).block(block), area);
    }
    let sb = &theme.status_bar;
    let bottom_bar = if let Some((done, total)) = app.save_progress {
        let pct = if total > 0 {
            done as f64 / total as f64
        } else {
            0.0
        };
        let bar_w = 20usize;
        let filled = (pct * bar_w as f64).round() as usize;
        let bar = format!(
            "{}{}",
            icons.progress_filled.repeat(filled.min(bar_w)),
            icons.progress_empty.repeat(bar_w.saturating_sub(filled))
        );
        let text = format!(" Saving... {}/{} {} ", done, total, bar);
        Paragraph::new(text).style(
            Style::default()
                .fg(sb.save_fg.0)
                .bg(sb.save_bg.0)
                .add_modifier(sb.save_modifier.0),
        )
    } else {
        match app.mode {
            AppMode::Normal => {
                let status_msg = app
                    .status_message_time
                    .filter(|t| t.elapsed() < Duration::from_secs(5))
                    .map(|_| app.status_message.clone());
                if let Some(msg) = status_msg {
                    Paragraph::new(format!(" {}", msg)).style(
                        Style::default()
                            .fg(sb.status_msg_fg.0)
                            .bg(sb.status_msg_bg.0)
                            .add_modifier(sb.status_msg_modifier.0),
                    )
                } else {
                    let sort_indicator = format!(
                        "[{}]",
                        match app.nav.sort_mode {
                            SortMode::ByName =>
                                if app.nav.sort_reverse {
                                    "Name▲"
                                } else {
                                    "Name▼"
                                },
                            SortMode::ByDate =>
                                if app.nav.sort_reverse {
                                    "Date▲"
                                } else {
                                    "Date▼"
                                },
                            SortMode::BySize =>
                                if app.nav.sort_reverse {
                                    "Size▲"
                                } else {
                                    "Size▼"
                                },
                            SortMode::ByType =>
                                if app.nav.sort_reverse {
                                    "Type▲"
                                } else {
                                    "Type▼"
                                },
                        }
                    );
                    let filter_display = if !app.nav.filter_input.is_empty() {
                        format!(" [Filter: {}]", app.nav.filter_input)
                    } else {
                        String::new()
                    };
                    let hidden_indicator = if app.nav.show_hidden {
                        " [H]"
                    } else {
                        ""
                    };
                        let status_text = if app.nav.selected_files.is_empty() {
                        format!(" {}{}{}  | [s]Sort | [b]Mark | [B]Bookm | [Ctrl+A]Act | [:]Cmd | [F1]Help | [q]Quit",sort_indicator, filter_display, hidden_indicator)
                    } else {
                        format!(" {}{}{} | Sel: {} | [:] cp mv rm", sort_indicator, filter_display, hidden_indicator, app.nav.selected_files.len())
                    };
                    Paragraph::new(status_text)
                        .style(Style::default().fg(sb.normal_fg.0).bg(sb.normal_bg.0))
                }
            }
            AppMode::Command => {
                let command_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(sb.command_border_fg.0))
                    .bg(sb.command_bg.0);
                let status_msg = app
                    .status_message_time
                    .filter(|t| t.elapsed() < Duration::from_secs(5))
                    .map(|_| app.status_message.clone());
                if let Some(ref msg) = status_msg {
                    Paragraph::new(format!(" {}", msg)).style(
                        Style::default()
                            .fg(sb.status_msg_fg.0)
                            .bg(sb.status_msg_bg.0)
                            .add_modifier(sb.status_msg_modifier.0),
                    )
                } else {
                    if app.cmd.command_input.starts_with(":grep ") {
                        let query = app.cmd.command_input.trim_start_matches(":grep ").trim();
                        let hint = crate::grep::Searcher::checker_pattern(query);
                        let hint_rect = Rect::new(chunks[3].x, chunks[3].y - 2, chunks[3].width, 1);
                        let hint_style = if hint.is_valid {
                            Style::default().fg(theme.grep_hint.valid_fg.0)
                        } else {
                            Style::default().fg(theme.grep_hint.invalid_fg.0)
                        };
                        f.render_widget(
                            Paragraph::new(format!(" 💡 {}", hint.message)).style(hint_style),
                            hint_rect,
                        );
                    }
                    Paragraph::new(app.cmd.command_input.clone())
                        .block(command_block)
                        .style(Style::default().fg(sb.command_fg.0).bg(sb.command_bg.0))
                }
            }
            AppMode::Search => {
                let count = app.nav.files.len();
                Paragraph::new(format!(" Search [{}]: {}  |  ↑/↓ navigate  Enter open  Esc close", count, app.search.search_query))
                    .style(Style::default().bg(sb.search_bg.0).fg(sb.search_fg.0))
            }
            AppMode::Breadcrumbs => {
                Paragraph::new(" [Left/Right] Navigate | [Enter] Go | [Down/Esc] To files")
                    .style(Style::default().fg(sb.breadcrumbs_fg.0).bg(sb.breadcrumbs_bg.0))
            }
            AppMode::BookMarks => Paragraph::new(
                " [Up/Down] Select bookmark | [Enter] Jump | [d] Delete | [Esc] Close",
            )
            .style(Style::default().fg(sb.bookmarks_fg.0).bg(sb.bookmarks_bg.0)),
            AppMode::GrepResults => {
                if let Some(selected_idx) = app.grep.grep_state.selected() {
                    let popup_vertical = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Percentage(10),
                            Constraint::Percentage(80),
                            Constraint::Percentage(10),
                        ])
                        .split(f.area());
                    let popup_area = popup_vertical[1];
                    let cursor_y =
                        popup_area.y + 1 + (selected_idx as u16).min(popup_area.height - 3);
                    let cursor_x = popup_area.x + 4;
                    f.set_cursor_position(Position::new(cursor_x, cursor_y));
                }
                Paragraph::new(" [Up/Down] Scroll matches | [Enter] Open file | [Esc] Close")
                    .style(Style::default().fg(sb.grep_results_fg.0).bg(sb.grep_results_bg.0))
            }
            AppMode::DiskUsage => {
                Paragraph::new(" [Up/Down] Browse | [Enter] Enter folder | [Esc] Exit")
                    .style(Style::default().fg(sb.disk_usage_fg.0).bg(sb.disk_usage_bg.0))
            }
            AppMode::ConfirmDelete => Paragraph::new(" Confirm delete: [y] Yes | [n/Esc] No")
                .style(
                    Style::default()
                        .fg(sb.confirm_delete_fg.0)
                        .bg(sb.confirm_delete_bg.0)
                        .add_modifier(sb.confirm_delete_modifier.0),
                ),
            AppMode::Viewer => {
                Paragraph::new(" [↑/↓/PgUp/PgDn] Scroll | [e/i] Edit | [v/Esc/q] Close")
                    .style(Style::default().fg(sb.viewer_fg.0).bg(sb.viewer_bg.0))
            }
            AppMode::Editor => {
                let modified = if app.editor.editor_modified {
                    " [Modified]"
                } else {
                    ""
                };
                Paragraph::new(format!(
                    " [↑↓←→] Move | [Ctrl+S] Save{} | [Esc] Exit",
                    modified
                ))
                .style(Style::default().fg(sb.editor_fg.0).bg(sb.editor_bg.0))
            }
            AppMode::FileInfo => Paragraph::new(" [Esc/q] Close  [Ctrl+C] Copy path to clipboard")
                .style(Style::default().fg(sb.file_info_fg.0).bg(sb.file_info_bg.0)),
            AppMode::Action => Paragraph::new(
                " [↑/↓] Select | [Enter] Execute | [Esc/q] Close",
            )
            .style(Style::default().fg(sb.action_fg.0).bg(sb.action_bg.0)),
            AppMode::Help => Paragraph::new(" [↑/↓/PgUp/PgDn] Scroll | [Esc/q/F1] Close")
                .style(Style::default().fg(sb.help_fg.0).bg(sb.help_bg.0)),
        }
    };
    f.render_widget(bottom_bar, chunks[3]);

    fn make_process(percent: f32, icons: &Icons) -> String {
        let total_bars = 15;
        let filled = ((percent / 100.0) * total_bars as f32).round() as usize;
        let filled = filled.min(total_bars);
        format!(
            "[{}{}] {:.0}%",
            icons.progress_filled.repeat(filled),
            icons.progress_empty.repeat(total_bars - filled),
            percent
        )
    }
}
