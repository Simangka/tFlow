use ratatui::{Frame, layout::Rect, style::Style, widgets::{Block, Paragraph}, text::Span};

use crate::theme::Theme;

pub struct ScrollbarRenderer;

impl ScrollbarRenderer {
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        total_lines: usize,
        visible_lines: usize,
        scroll_offset: usize,
        theme: &Theme,
    ) {
        Self::render_vertical(frame, area, scroll_offset, total_lines, visible_lines, theme);
    }

    pub fn render_vertical(
        frame: &mut Frame,
        area: Rect,
        first_visible: usize,
        total: usize,
        visible: usize,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 || total == 0 || visible == 0 {
            return;
        }

        if visible >= total {
            let bg_style = Style::default().bg(theme.scrollbar_bg);
            let block = Block::default().style(bg_style);
            frame.render_widget(block, area);
            return;
        }

        let scrollbar_height = area.height as usize;
        let thumb_height = ((visible as f64 / total as f64) * scrollbar_height as f64).ceil() as usize;
        let thumb_height = thumb_height.max(1).min(scrollbar_height);

        let max_thumb_pos = scrollbar_height.saturating_sub(thumb_height);
        let scroll_fraction = first_visible as f64 / (total.saturating_sub(visible)) as f64;
        let thumb_pos = (scroll_fraction * max_thumb_pos as f64).round() as usize;
        let thumb_pos = thumb_pos.min(max_thumb_pos);

        let mut lines: Vec<ratatui::text::Line<'static>> = Vec::with_capacity(scrollbar_height);

        for i in 0..scrollbar_height {
            if i >= thumb_pos && i < thumb_pos + thumb_height {
                let thumb_char = "█";
                let padded = format!("{:width$}", thumb_char, width = area.width as usize);
                let span = Span::styled(padded, Style::default().fg(theme.scrollbar));
                lines.push(ratatui::text::Line::from(vec![span]));
            } else {
                let padding = " ".repeat(area.width as usize);
                let span = Span::styled(padding, Style::default().bg(theme.scrollbar_bg));
                lines.push(ratatui::text::Line::from(vec![span]));
            }
        }

        let paragraph = Paragraph::new(lines).style(Style::default().bg(theme.scrollbar_bg));
        frame.render_widget(paragraph, area);
    }
}
