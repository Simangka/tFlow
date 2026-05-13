use ratatui::{Frame, layout::Rect, widgets::Paragraph, style::Style, text::Line as TextLine, text::Span};

use crate::theme::Theme;

pub struct LineNumbers;

impl LineNumbers {
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        visible_lines: std::ops::Range<usize>,
        current_line: usize,
        theme: &Theme,
        relative: bool,
        first_line_number: usize,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut lines: Vec<TextLine<'static>> = Vec::new();

        for (_i, line_idx) in visible_lines.clone().enumerate() {
            let line_num = first_line_number + line_idx;
            let is_active = line_idx == current_line;
            let formatted = Self::format_line_number(line_num, line_idx, current_line, relative);

            let style = if is_active {
                Style::default().fg(theme.line_numbers_active)
            } else {
                Style::default().fg(theme.line_numbers)
            };

            let padded = Self::format_padded(&formatted, area.width as usize);
            let span = Span::styled(padded, style);
            lines.push(TextLine::from(vec![span]));
        }

        let empty_count = (area.height as usize).saturating_sub(lines.len());
        for _ in 0..empty_count {
            let padding = " ".repeat(area.width as usize);
            let span = Span::styled(padding, Style::default().fg(theme.line_numbers));
            lines.push(TextLine::from(vec![span]));
        }

        let paragraph = Paragraph::new(lines).style(Style::default().bg(theme.bg));
        frame.render_widget(paragraph, area);
    }

    pub fn gutter_width(line_count: usize) -> u16 {
        let digits = if line_count == 0 { 1 } else { line_count.ilog10() as u16 + 1 };
        digits + 2
    }

    fn format_padded(num: &str, width: usize) -> String {
        if num.len() >= width {
            format!("{} ", &num[..width.saturating_sub(1)])
        } else {
            let padding = width.saturating_sub(num.len()).saturating_sub(1);
            format!("{}{} ", " ".repeat(padding), num)
        }
    }

    pub fn format_line_number(line: usize, _idx: usize, current_line: usize, relative: bool) -> String {
        if relative {
            if line == current_line + 1 {
                line.to_string()
            } else {
                let diff = if line > current_line + 1 {
                    line - current_line - 1
                } else {
                    current_line + 1 - line
                };
                diff.to_string()
            }
        } else {
            line.to_string()
        }
    }
}
