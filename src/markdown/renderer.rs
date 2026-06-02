use ratatui::style::{Style, Modifier};
use crate::theme::Theme;
use super::prepare_md;

const MAX_CODE_LINES: usize = 10_000;

#[derive(Clone)]
pub struct MarkdownRenderLine {
    pub content: String,
    pub style: Style,
    pub indent: u16,
    pub is_heading: bool,
    pub heading_level: u8,
}

pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn render(text: &str, theme: &Theme) -> Vec<MarkdownRenderLine> {
        let prepared = prepare_md(text);
        let text: &str = &prepared;
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();
        let mut result = Vec::with_capacity(total);
        let default_style = Style::default().fg(theme.fg).bg(theme.bg);

        let mut i = 0;
        while i < total {
            let raw = lines[i];
            let trimmed = raw.trim();

            if raw.is_empty() {
                result.push(MarkdownRenderLine {
                    content: String::new(),
                    style: default_style,
                    indent: 0, is_heading: false, heading_level: 0,
                });
                i += 1;
                continue;
            }

            if let Some(level) = heading_prefix(trimmed) {
                let color = match level {
                    1 => theme.heading1,
                    2 => theme.heading2,
                    3 => theme.heading3,
                    _ => theme.heading3,
                };
                let style = Style::default().fg(color).bg(theme.bg).add_modifier(Modifier::BOLD);
                result.push(MarkdownRenderLine {
                    content: raw.to_string(),
                    style,
                    indent: 0, is_heading: true, heading_level: level,
                });
                i += 1;
                continue;
            }

            if trimmed.starts_with("---") && trimmed.trim_matches('-').is_empty() {
                result.push(MarkdownRenderLine {
                    content: raw.to_string(),
                    style: Style::default().fg(theme.comment).bg(theme.bg),
                    indent: 0, is_heading: false, heading_level: 0,
                });
                i += 1;
                continue;
            }

            if trimmed.starts_with("> ") || trimmed.starts_with('>') {
                let content = raw.to_string();
                let style = Style::default().fg(theme.blockquote).bg(theme.bg).add_modifier(Modifier::ITALIC);
                result.push(MarkdownRenderLine {
                    content,
                    style,
                    indent: 0, is_heading: false, heading_level: 0,
                });
                i += 1;
                continue;
            }

            if raw.starts_with("```") {
                let _lang = raw.trim_start_matches('`').trim();
                let mut code_lines = vec![MarkdownRenderLine {
                    content: raw.to_string(),
                    style: Style::default().fg(theme.code_block).bg(theme.bg),
                    indent: 0, is_heading: false, heading_level: 0,
                }];
                i += 1;
                let mut count: usize = 0;
                while i < total {
                    if count >= MAX_CODE_LINES {
                        code_lines.push(MarkdownRenderLine {
                            content: "[truncated]".to_string(),
                            style: Style::default().fg(theme.comment).bg(theme.bg),
                            indent: 0, is_heading: false, heading_level: 0,
                        });
                        break;
                    }
                    if lines[i].starts_with("```") {
                        code_lines.push(MarkdownRenderLine {
                            content: lines[i].to_string(),
                            style: Style::default().fg(theme.code_block).bg(theme.bg),
                            indent: 0, is_heading: false, heading_level: 0,
                        });
                        i += 1;
                        break;
                    }
                    code_lines.push(MarkdownRenderLine {
                        content: lines[i].to_string(),
                        style: Style::default().fg(theme.code_block).bg(theme.bg),
                        indent: 0, is_heading: false, heading_level: 0,
                    });
                    i += 1;
                    count += 1;
                }
                result.extend(code_lines);
                continue;
            }

            if let Some(prefix) = list_item_prefix(trimmed) {
                let indent = raw.len() - raw.trim_start().len();
                let style = Style::default().fg(theme.list).bg(theme.bg);
                let display = if prefix.starts_with("- [") {
                    let checked = prefix.contains("[x]");
                    let marker = if checked { "[x]" } else { "[ ]" };
                    let prefix_len = prefix.len();
                    if raw.len() > prefix_len {
                        format!("{}{}", marker, &raw[prefix_len..])
                    } else {
                        raw.to_string()
                    }
                } else {
                    raw.to_string()
                };
                result.push(MarkdownRenderLine {
                    content: display,
                    style,
                    indent: indent as u16,
                    is_heading: false, heading_level: 0,
                });
                i += 1;
                continue;
            }

            let is_table = raw.contains('|') && raw.chars().filter(|&c| c == '|').count() >= 2;
            if is_table {
                let style = Style::default().fg(theme.fg).bg(theme.bg);
                result.push(MarkdownRenderLine {
                    content: raw.to_string(),
                    style,
                    indent: 0, is_heading: false, heading_level: 0,
                });
                i += 1;
                continue;
            }

            let style = Style::default().fg(theme.fg).bg(theme.bg);
            result.push(MarkdownRenderLine {
                content: raw.to_string(),
                style,
                indent: 0, is_heading: false, heading_level: 0,
            });
            i += 1;
        }

        result
    }
}

fn heading_prefix(s: &str) -> Option<u8> {
    let t = s.trim_start();
    if t.starts_with('#') {
        let count = t.chars().take_while(|&c| c == '#').count();
        if count <= 6 && t.len() > count && t.as_bytes()[count] == b' ' {
            return Some(count as u8);
        }
    }
    None
}

fn list_item_prefix(s: &str) -> Option<String> {
    let t = s.trim_start();
    if t.starts_with("- [ ] ") {
        return Some("- [ ] ".to_string());
    }
    if t.starts_with("- [x] ") || t.starts_with("- [X] ") {
        return Some("- [x] ".to_string());
    }
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        let bullet = t.chars().next().unwrap();
        return Some(format!("{} ", bullet));
    }
    let prefix: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ')' || *c == ' ').collect();
    if prefix.chars().any(|c| c.is_ascii_digit()) {
        Some(format!("{} ", prefix.trim_end()))
    } else {
        None
    }
}
