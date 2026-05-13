use ratatui::style::Style;
use crate::theme::Theme;
use super::renderer::MarkdownRenderLine;

pub struct PlainTextRenderer;

impl PlainTextRenderer {
    pub fn render(text: &str, theme: &Theme) -> Vec<MarkdownRenderLine> {
        let style = Style::default().fg(theme.fg).bg(theme.bg);
        text.lines().map(|line| MarkdownRenderLine {
            content: line.to_string(),
            style,
            indent: 0,
            is_heading: false,
            heading_level: 0,
        }).collect()
    }
}
