use crate::theme::MarkdownTheme;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    prelude::*,
    text::{Line, Span},
};
pub fn markdown_parser(text: &str, md: &MarkdownTheme) -> Vec<Line<'static>> {
    let parser = Parser::new(text);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = Vec::new();
    let mut code_block = false;

    let flush_line = |spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>| {
        if !spans.is_empty() {
            lines.push(Line::from(std::mem::take(spans)));
        } else {
            lines.push(Line::from(""))
        }
    };
    let current_style = |stack: &[Style]| {
        let mut s = Style::default();
        for style in stack.iter().rev() {
            s = s.patch(*style);
        }
        s
    };
    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level: _, .. } => {
                    style_stack.push(
                        Style::default()
                            .fg(md.heading_fg.0)
                            .add_modifier(md.heading_modifier.0),
                    );
                }
                Tag::Paragraph => {}
                Tag::Emphasis => style_stack.push(Style::default().add_modifier(md.italic_modifier.0)),
                Tag::Strong => style_stack.push(Style::default().add_modifier(md.bold_modifier.0)),
                Tag::CodeBlock(_) => {
                    code_block = true;
                    style_stack.push(Style::default().fg(md.code_fg.0).bg(md.code_bg.0));
                }
                Tag::List(_) => {}
                Tag::Item => {}
                Tag::Link { dest_url: _, .. } => {
                    style_stack.push(
                        Style::default()
                            .fg(md.link_fg.0)
                            .add_modifier(md.link_modifier.0),
                    );
                }
                Tag::BlockQuote(_) => {
                    style_stack.push(
                        Style::default()
                            .fg(md.blockquote_fg.0)
                            .add_modifier(md.blockquote_modifier.0),
                    );
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    flush_line(&mut current_spans, &mut lines);
                }
                TagEnd::Paragraph => {
                    flush_line(&mut current_spans, &mut lines);
                }
                TagEnd::Emphasis => {
                    style_stack.pop();
                }
                TagEnd::Strong => {
                    style_stack.pop();
                }
                TagEnd::CodeBlock => {
                    style_stack.pop();
                    code_block = false;
                    flush_line(&mut current_spans, &mut lines);
                }
                TagEnd::Item => {
                    flush_line(&mut current_spans, &mut lines);
                }
                TagEnd::Link => {
                    style_stack.pop();
                }
                TagEnd::BlockQuote(_) => {
                    style_stack.pop();
                }
                _ => {}
            },
            Event::Text(text) => {
                let s = current_style(&style_stack);
                let text_str = text.to_string();
                if code_block {
                    for line in text_str.lines() {
                        current_spans.push(Span::styled(line.to_string(), s));
                        flush_line(&mut current_spans, &mut lines);
                    }
                } else {
                    current_spans.push(Span::styled(text_str, s));
                }
            }
            Event::Code(text) => {
                let style = Style::default().fg(md.inline_code_fg.0);
                current_spans.push(Span::styled(text.to_string(), style));
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_line(&mut current_spans, &mut lines);
            }
            Event::Rule => {
                const WIDTH: usize = 50;
                current_spans.push(Span::styled(
                    "-".repeat(WIDTH),
                    Style::default().fg(md.hr_fg.0),
                ));
                flush_line(&mut current_spans, &mut lines);
            }
            _ => {}
        }
    }
    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }
    lines
}