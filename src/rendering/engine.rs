use ratatui::{
    layout::Rect,
    Frame,
    widgets::Paragraph,
    text::{Line as TextLine, Span},
    style::Style,
};

use crate::core::Position;
use crate::core::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::theme::Theme;

#[derive(Debug, Clone)]
pub struct RenderEngine {
    pub use_dirty_regions: bool,
    pub dirty_regions: Vec<Rect>,
    pub prev_viewport: Option<ViewportState>,
    pub scroll_offset: Position,
    pub viewport_height: usize,
    pub viewport_width: usize,
}

#[derive(Debug, Clone)]
pub struct ViewportState {
    pub first_line: usize,
    pub last_line: usize,
    pub scroll_offset: usize,
}

impl RenderEngine {
    pub fn new() -> Self {
        Self {
            use_dirty_regions: true,
            dirty_regions: Vec::new(),
            prev_viewport: None,
            scroll_offset: Position::zero(),
            viewport_height: 0,
            viewport_width: 0,
        }
    }

    pub fn mark_dirty(&mut self, area: &Rect) {
        self.dirty_regions.push(*area);
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_regions.clear();
    }

    pub fn render_buffer(
        &self,
        frame: &mut Frame,
        area: Rect,
        buffer: &Buffer,
        cursor: &Cursor,
        selection: &Selection,
        theme: &Theme,
        line_numbers: bool,
        relative_numbers: bool,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let total_lines = buffer.line_count();
        if total_lines == 0 {
            return;
        }

        let gutter_width_val = if line_numbers {
            crate::rendering::line_numbers::LineNumbers::gutter_width(total_lines)
        } else {
            0
        };
        let text_area = Rect::new(
            area.x + gutter_width_val,
            area.y,
            area.width.saturating_sub(gutter_width_val),
            area.height,
        );

        let scroll_line = self.scroll_offset.line;
        let visible_start = scroll_line;
        let visible_end = (scroll_line + area.height as usize).min(total_lines);

        if visible_start >= visible_end {
            return;
        }

        let visible_range = visible_start..visible_end;
        let current_line = cursor.position.line;

        if line_numbers {
            crate::rendering::line_numbers::LineNumbers::render(
                frame,
                Rect::new(area.x, area.y, gutter_width_val, area.height),
                visible_range.clone(),
                current_line,
                theme,
                relative_numbers,
                visible_start + 1,
            );
        }

        let col_offset = self.scroll_offset.column;
        let mut text_lines: Vec<TextLine<'static>> = Vec::with_capacity(visible_range.len());

        for line_idx in visible_range.clone() {
            let line_text = if line_idx < total_lines {
                buffer.get_line(line_idx)
            } else {
                String::new()
            };

            let is_active_line = line_idx == current_line;
            let line_bg = if is_active_line { Some(theme.cursor_line) } else { None };

            let mut spans: Vec<Span<'static>> = Vec::new();
            let sel_range = selection.normalized_range();

            if let Some(ref sr) = sel_range {
                if !sr.is_empty() && line_idx >= sr.start.line && line_idx <= sr.end.line {
                    let sel_start_col = if line_idx == sr.start.line { sr.start.column } else { 0 };
                    let sel_end_col = if line_idx == sr.end.line { sr.end.column } else { line_text.len() };

                    let sel_start_clamped = sel_start_col.min(line_text.len());
                    let sel_end_clamped = sel_end_col.min(line_text.len());

                    if sel_start_clamped > 0 {
                        let before = &line_text[..sel_start_clamped];
                        if !before.is_empty() {
                            spans.push(Span::styled(before.to_string(), Style::default().fg(theme.fg)));
                        }
                    }

                    if sel_end_clamped > sel_start_clamped {
                        let selected = &line_text[sel_start_clamped..sel_end_clamped];
                        if !selected.is_empty() {
                            spans.push(Span::styled(selected.to_string(), theme.selection_style()));
                        }
                    }

                    if sel_end_clamped < line_text.len() {
                        let after = &line_text[sel_end_clamped..];
                        if !after.is_empty() {
                            spans.push(Span::styled(after.to_string(), Style::default().fg(theme.fg)));
                        }
                    }

                    if spans.is_empty() {
                        let display = &line_text[col_offset.min(line_text.len())..];
                        spans.push(Span::styled(display.to_string(), Style::default().fg(theme.fg)));
                    }

                    let mut line_style = Style::default().fg(theme.fg);
                    if let Some(bg) = line_bg {
                        line_style = line_style.bg(bg);
                    }
                    text_lines.push(TextLine::from(spans).style(line_style));
                    continue;
                }
            }

            let display_text = if col_offset < line_text.len() {
                line_text[col_offset..].to_string()
            } else {
                String::new()
            };

            let mut line_style = Style::default().fg(theme.fg);
            if let Some(bg) = line_bg {
                line_style = line_style.bg(bg);
            }

            if display_text.is_empty() {
                text_lines.push(TextLine::from(vec![Span::styled(" ", line_style)]));
            } else {
                spans.push(Span::styled(display_text, line_style));
                text_lines.push(TextLine::from(spans).style(line_style));
            }
        }

        let visible_count = text_lines.len();
        for _ in 0..(area.height as usize).saturating_sub(visible_count) {
            text_lines.push(TextLine::from(vec![Span::styled(
                " ",
                Style::default().fg(theme.fg),
            )]));
        }

        let paragraph = Paragraph::new(text_lines).style(Style::default().bg(theme.bg));
        frame.render_widget(paragraph, text_area);
    }

    pub fn render_cursor(
        &self,
        frame: &mut Frame,
        area: Rect,
        cursor: &Cursor,
        _theme: &Theme,
    ) {
        let cursor_line = cursor.position.line;
        let scroll_line = self.scroll_offset.line;
        if cursor_line < scroll_line {
            return;
        }
        let visual_line = cursor_line - scroll_line;
        if visual_line >= self.viewport_height {
            return;
        }

        let col = cursor.position.column.saturating_sub(self.scroll_offset.column);
        let cursor_x = area.x + col as u16;
        let cursor_y = area.y + visual_line as u16;

        let max_x = area.x + area.width.saturating_sub(1);
        let max_y = area.y + area.height.saturating_sub(1);
        let final_x = cursor_x.min(max_x);
        let final_y = cursor_y.min(max_y);

        frame.set_cursor_position(ratatui::layout::Position::new(final_x, final_y));
    }

    pub fn render_line(
        &self,
        frame: &mut Frame,
        area: Rect,
        line: &str,
        line_idx: usize,
        col_offset: usize,
        _cursor: &Cursor,
        selection: &Selection,
        theme: &Theme,
        _buffer: &Buffer,
        is_active_line: bool,
    ) -> u16 {
        if area.width == 0 || area.height == 0 {
            return 0;
        }

        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut line_style = Style::default().fg(theme.fg);
        if is_active_line {
            line_style = line_style.bg(theme.cursor_line);
        }

        let sel_range = selection.normalized_range();
        if let Some(ref sr) = sel_range {
            if !sr.is_empty() && line_idx >= sr.start.line && line_idx <= sr.end.line {
                let sel_start_col = if line_idx == sr.start.line { sr.start.column } else { 0 };
                let sel_end_col = if line_idx == sr.end.line { sr.end.column } else { line.len() };

                let sel_start = sel_start_col.min(line.len());
                let sel_end = sel_end_col.min(line.len());

                if col_offset < sel_start {
                    let before = &line[col_offset..sel_start];
                    if !before.is_empty() {
                        spans.push(Span::styled(before.to_string(), line_style));
                    }
                }

                let sel_effective_start = if col_offset > sel_start { col_offset } else { sel_start };
                if sel_effective_start < sel_end {
                    let selected = &line[sel_effective_start..sel_end];
                    if !selected.is_empty() {
                        spans.push(Span::styled(selected.to_string(), theme.selection_style()));
                    }
                }

                if sel_end < line.len() {
                    let after = &line[sel_end..];
                    if !after.is_empty() {
                        spans.push(Span::styled(after.to_string(), line_style));
                    }
                }

                if spans.is_empty() {
                    let text = &line[col_offset.min(line.len())..];
                    spans.push(Span::styled(text.to_string(), line_style));
                }
            } else {
                let text = &line[col_offset.min(line.len())..];
                spans.push(Span::styled(text.to_string(), line_style));
            }
        } else {
            let text = &line[col_offset.min(line.len())..];
            spans.push(Span::styled(text.to_string(), line_style));
        }

        if spans.is_empty() {
            spans.push(Span::styled(" ", line_style));
        }

        let text_line = TextLine::from(spans).style(line_style);
        let paragraph = Paragraph::new(vec![text_line]).style(Style::default().bg(theme.bg));
        frame.render_widget(paragraph, area);

        line.len() as u16
    }

    pub fn render_text_highlighted(
        &self,
        frame: &mut Frame,
        area: Rect,
        text: &str,
        style: ratatui::style::Style,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let span = Span::styled(text.to_string(), style);
        let line = TextLine::from(vec![span]);
        let paragraph = Paragraph::new(vec![line]).style(style);
        frame.render_widget(paragraph, area);
    }

    pub fn compute_visible_lines(&self, buffer: &Buffer) -> std::ops::Range<usize> {
        let total = buffer.line_count();
        let start = self.scroll_offset.line.min(total);
        let end = (start + self.viewport_height).min(total);
        start..end
    }

    pub fn set_viewport(&mut self, height: usize, width: usize) {
        self.viewport_height = height;
        self.viewport_width = width;
    }

    pub fn scroll_to(&mut self, pos: Position, buffer: &Buffer) {
        let total = buffer.line_count();
        let max_scroll = total.saturating_sub(self.viewport_height);
        let line = pos.line.min(max_scroll);
        self.scroll_offset = Position::new(line, pos.column);
    }

    pub fn scroll_delta(&mut self, delta_line: isize, buffer: &Buffer) {
        let total = buffer.line_count();
        let current = self.scroll_offset.line as isize;
        let new_line = (current + delta_line).max(0) as usize;
        let max_scroll = total.saturating_sub(self.viewport_height);
        let clamped_line = new_line.min(max_scroll);
        self.scroll_offset.line = clamped_line;
    }
}
