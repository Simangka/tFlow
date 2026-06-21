use std::cell::{Cell, RefCell};

use ratatui::{
    layout::Rect,
    Frame,
    widgets::Paragraph,
    text::{Line as TextLine, Span},
    style::{Style, Color},
};

use crate::core::{Position, SearchState};
use crate::core::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::theme::Theme;
use crate::theme::syntax::SyntaxHighlighter;
use ratatui::style::Modifier;

#[derive(Debug, Clone)]
pub struct RenderEngine {
    pub use_dirty_regions: bool,
    pub dirty_regions: Vec<Rect>,
    pub prev_viewport: Option<ViewportState>,
    pub scroll_offset: Position,
    pub viewport_height: usize,
    pub viewport_width: usize,
    pub word_wrap: bool,
    // P2.1: Cache visual_to_logical result within a render frame (Cell for interior mutability)
    pub visual_line_cache: Cell<Option<(usize, usize, usize, usize)>>,
    // P2.3: Cache search matches by line across frames (RefCell for interior mutability)
    pub cached_search_query: RefCell<String>,
    pub cached_search_matches: RefCell<Vec<Vec<(usize, usize)>>>,
    pub cached_search_matches_snapshot: RefCell<Vec<Position>>,
    // P2.2: Dirty-flag throttling for event loop (Cell for interior mutability)
    pub dirty: Cell<bool>,
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
            word_wrap: true,
            visual_line_cache: Cell::new(None),
            cached_search_query: RefCell::new(String::new()),
            cached_search_matches: RefCell::new(Vec::new()),
            cached_search_matches_snapshot: RefCell::new(Vec::new()),
            dirty: Cell::new(true),
        }
    }

    pub fn mark_dirty(&mut self, area: &Rect) {
        self.dirty_regions.push(*area);
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_regions.clear();
    }

    /// Returns true if the engine requires rendering (dirty flag is set).
    pub fn needs_render(&self) -> bool {
        self.dirty.get()
    }

    /// Find a break point at a word boundary within `max_width` chars.
    fn find_wrap_break(&self, text: &str, max_width: usize) -> usize {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= max_width {
            return text.len();
        }
        let mut break_at = max_width;
        while break_at > 0 && !chars[break_at - 1].is_whitespace() {
            break_at -= 1;
        }
        if break_at == 0 {
            text.char_indices().nth(max_width).map(|(i, _)| i).unwrap_or(text.len())
        } else {
            text.char_indices().nth(break_at).map(|(i, _)| i).unwrap_or(text.len())
        }
    }

    /// Split text into visual line segments at word boundaries.
    pub fn wrap_text<'a>(&self, text: &'a str, max_width: usize) -> Vec<&'a str> {
        if !self.word_wrap || max_width < 2 {
            return vec![text];
        }
        let mut lines = Vec::new();
        let mut remaining = text;
        while !remaining.is_empty() {
            let char_len = remaining.chars().count();
            if char_len <= max_width {
                lines.push(remaining);
                break;
            }
            let break_at = self.find_wrap_break(remaining, max_width);
            lines.push(&remaining[..break_at]);
            remaining = &remaining[break_at..];
        }
        if lines.is_empty() {
            lines.push(text);
        }
        lines
    }

    /// Count how many visual lines a single logical line produces.
    pub fn wrapped_line_count(&self, text: &str, max_width: usize) -> usize {
        if !self.word_wrap || max_width < 2 {
            return 1;
        }
        let len = text.chars().count();
        if len == 0 { return 1; }
        if len <= max_width { return 1; }
        let mut count = 0;
        let mut remaining = text;
        while !remaining.is_empty() {
            let char_len = remaining.chars().count();
            if char_len <= max_width {
                count += 1;
                break;
            }
            let break_at = self.find_wrap_break(remaining, max_width);
            remaining = &remaining[break_at..];
            count += 1;
        }
        count.max(1)
    }

    /// Map a logical (line, col) to visual (line, col) accounting for word wrap.
    pub fn logical_to_visual(&self, buffer: &Buffer, pos: Position, max_width: usize) -> Position {
        if !self.word_wrap || max_width < 2 {
            return pos;
        }
        let line_text = buffer.get_line(pos.line);
        let target_col = pos.column;
        let mut visual_line_offset = 0;
        let mut remaining = &line_text[..];
        let mut accumulated = 0;

        while !remaining.is_empty() {
            let char_len = remaining.chars().count();
            if char_len <= max_width {
                return Position::new(pos.line + visual_line_offset, target_col - accumulated);
            }
            let break_at = self.find_wrap_break(remaining, max_width);
            let seg_chars = remaining[..break_at].chars().count();
            if target_col < accumulated + seg_chars {
                return Position::new(pos.line + visual_line_offset, target_col - accumulated);
            }
            accumulated += seg_chars;
            visual_line_offset += 1;
            remaining = &remaining[break_at..];
        }
        Position::new(pos.line + visual_line_offset, 0)
    }

    /// Total visual lines in the buffer with current wrap settings.
    pub fn total_visual_lines(&self, buffer: &Buffer, max_width: usize) -> usize {
        if !self.word_wrap || max_width < 2 {
            return buffer.line_count();
        }
        let mut count = 0;
        for i in 0..buffer.line_count() {
            let line = buffer.get_line_ref(i);
            count += self.wrapped_line_count(line, max_width);
        }
        count
    }

    /// Given a visual line index, find which logical line and segment offset it corresponds to.
    /// Uses an internal cache to avoid repeated linear scans within the same frame.
    pub fn visual_to_logical(&self, buffer: &Buffer, visual_line: usize, max_width: usize) -> (usize, usize) {
        // Check cache: (visual_line, max_width) -> (logical_line, segment_offset)
        if let Some((cv, cm, cl, cs)) = self.visual_line_cache.get() {
            if cv == visual_line && cm == max_width {
                return (cl, cs);
            }
        }
        if !self.word_wrap || max_width < 2 {
            let result = (visual_line, 0);
            self.visual_line_cache.set(Some((visual_line, max_width, result.0, result.1)));
            return result;
        }
        let mut accumulated = 0;
        for i in 0..buffer.line_count() {
            let line = buffer.get_line_ref(i);
            let count = self.wrapped_line_count(line, max_width);
            if visual_line < accumulated + count {
                let result = (i, visual_line - accumulated);
                self.visual_line_cache.set(Some((visual_line, max_width, result.0, result.1)));
                return result;
            }
            accumulated += count;
        }
        let result = (buffer.line_count().saturating_sub(1), 0);
        self.visual_line_cache.set(Some((visual_line, max_width, result.0, result.1)));
        result
    }

    /// Slice syntax-highlighted spans to a substring range [char_start, char_end).
    fn slice_spans(spans: &[(String, Style)], char_start: usize, char_end: usize) -> Vec<(String, Style)> {
        if char_start >= char_end || spans.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut accumulated = 0;
        for (text, style) in spans {
            let text_len = text.chars().count();
            let seg_start = accumulated;
            let seg_end = accumulated + text_len;
            if seg_end <= char_start {
                accumulated = seg_end;
                continue;
            }
            if seg_start >= char_end {
                break;
            }
            let local_start = char_start.saturating_sub(seg_start);
            let local_end = (char_end - seg_start).min(text_len);
            if local_start >= local_end {
                accumulated = seg_end;
                continue;
            }
            let start_byte = text.char_indices().nth(local_start).map(|(i, _)| i).unwrap_or(text.len());
            let end_byte = text.char_indices().nth(local_end).map(|(i, _)| i).unwrap_or(text.len());
            result.push((text[start_byte..end_byte].to_string(), *style));
            accumulated = seg_end;
        }
        result
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
        syntax_ext: Option<&str>,
        search_state: &SearchState,
        blame_data: Option<&[Option<crate::git::BlameInfo>]>,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let total_logical_lines = buffer.line_count();
        if total_logical_lines == 0 {
            return;
        }

        let gutter_width_val = if line_numbers {
            crate::rendering::line_numbers::LineNumbers::gutter_width(total_logical_lines)
        } else {
            0
        };
        let blame_width = if let Some(bd) = blame_data {
            if !bd.is_empty() { crate::rendering::blame_gutter::compute_width(bd) } else { 0 }
        } else { 0 };
        let text_area = Rect::new(
            area.x + gutter_width_val + blame_width,
            area.y,
            area.width.saturating_sub(gutter_width_val + blame_width),
            area.height,
        );
        let max_width = text_area.width as usize;

        let total_visual = self.total_visual_lines(buffer, max_width);
        let scroll_line = self.scroll_offset.line.min(total_visual.saturating_sub(1));
        let visible_start = scroll_line;
        let visible_end = (scroll_line + area.height as usize).min(total_visual);

        if visible_start >= visible_end {
            return;
        }

        // P2.1: Compute visual_to_logical values once and reuse
        let logical_line_start = self.visual_to_logical(buffer, visible_start, max_width).0;
        let logical_line_end = self.visual_to_logical(buffer, visible_end.saturating_sub(1), max_width).0 + 1;

        // Render line numbers gutter (logical line numbers)
        if line_numbers {
            let ln_range = logical_line_start..logical_line_end;
            crate::rendering::line_numbers::LineNumbers::render(
                frame,
                Rect::new(area.x, area.y, gutter_width_val, area.height),
                ln_range,
                cursor.position.line,
                theme,
                relative_numbers,
                logical_line_start + 1,
            );
        }

        if blame_width > 0 {
            if let Some(bd) = blame_data {
                let blame_area = Rect::new(
                    area.x + gutter_width_val,
                    area.y,
                    blame_width,
                    area.height,
                );
                let visible_blame: Vec<Option<crate::git::BlameInfo>> = bd.iter()
                    .skip(logical_line_start)
                    .take(area.height as usize)
                    .cloned()
                    .collect();
                crate::rendering::blame_gutter::render(frame, blame_area, &visible_blame, visible_start, theme);
            }
        }

        // P2.3: Reuse cached matches_by_line across frames when query/matches haven't changed
        let query_len = search_state.query.len();
        let needs_rebuild;
        {
            let cached_query = self.cached_search_query.borrow();
            let cached_snapshot = self.cached_search_matches_snapshot.borrow();
            needs_rebuild = *cached_query != search_state.query || *cached_snapshot != search_state.matches;
        }
        if needs_rebuild {
            let mut matches_by_line: Vec<Vec<(usize, usize)>> = vec![Vec::new(); total_logical_lines];
            if !search_state.query.is_empty() {
                for m in &search_state.matches {
                    let end = m.column + query_len;
                    if m.line < total_logical_lines {
                        matches_by_line[m.line].push((m.column, end));
                    }
                }
            }
            *self.cached_search_query.borrow_mut() = search_state.query.clone();
            *self.cached_search_matches.borrow_mut() = matches_by_line.clone();
            *self.cached_search_matches_snapshot.borrow_mut() = search_state.matches.clone();
        }

        let mut text_lines: Vec<TextLine<'static>> = Vec::with_capacity(area.height as usize);
        let mut current_visual = 0usize;

        // Hold a borrow on cached matches to avoid repeated RefCell lookups
        let matches_by_line_cache = self.cached_search_matches.borrow();

        for logical_line in 0..total_logical_lines {
            let line_text = buffer.get_line(logical_line);
            let segments = self.wrap_text(&line_text, max_width);
            let is_cursor_line = logical_line == cursor.position.line;

            for (seg_idx, segment) in segments.iter().enumerate() {
                if current_visual >= visible_end {
                    break;
                }
                if current_visual >= visible_start {
                    let seg_len = segment.chars().count();
                    let mut spans: Vec<Span<'static>> = Vec::new();

                    // Selection highlighting
                    let sel_range = selection.normalized_range();
                    let mut sel_handled = false;
                    if let Some(ref sr) = sel_range {
                        if !sr.is_empty() && logical_line >= sr.start.line && logical_line <= sr.end.line {
                            // Compute character offset of this segment within the logical line
                            let seg_offset: usize = segments[..seg_idx].iter().map(|s| s.chars().count()).sum();
                            let seg_end_offset = seg_offset + seg_len;

                            let sel_start_col = if logical_line == sr.start.line { sr.start.column } else { 0 };
                            let sel_end_col = if logical_line == sr.end.line { sr.end.column } else { line_text.len() };
                            let sel_start_clamped = sel_start_col.min(line_text.len());
                            let sel_end_clamped = sel_end_col.min(line_text.len());

                            if seg_end_offset > sel_start_clamped && seg_offset < sel_end_clamped {
                                let local_sel_start = sel_start_clamped.saturating_sub(seg_offset).min(seg_len);
                                let local_sel_end = sel_end_clamped.saturating_sub(seg_offset).min(seg_len);

                                if local_sel_start > 0 {
                                    let before = &segment[..segment.char_indices().nth(local_sel_start).map(|(i,_)| i).unwrap_or(segment.len())];
                                    if !before.is_empty() {
                                        spans.push(Span::styled(before.to_string(), Style::default().fg(theme.fg)));
                                    }
                                }
                                if local_sel_end > local_sel_start {
                                    let selected = &segment[segment.char_indices().nth(local_sel_start).map(|(i,_)| i).unwrap_or(0)
                                        ..segment.char_indices().nth(local_sel_end).map(|(i,_)| i).unwrap_or(segment.len())];
                                    if !selected.is_empty() {
                                        spans.push(Span::styled(selected.to_string(), theme.selection_style()));
                                    }
                                }
                                if local_sel_end < seg_len {
                                    let after = &segment[segment.char_indices().nth(local_sel_end).map(|(i,_)| i).unwrap_or(segment.len())..];
                                    if !after.is_empty() {
                                        spans.push(Span::styled(after.to_string(), Style::default().fg(theme.fg)));
                                    }
                                }
                                sel_handled = true;
                            }
                        }
                    }
                    if !sel_handled {
                        let matches_on_line = if query_len > 0 && logical_line < matches_by_line_cache.len() {
                            matches_by_line_cache[logical_line].clone()
                        } else {
                            Vec::new()
                        };

                        let current_match_on_line = if let Some(cmi) = search_state.current_match {
                            if cmi < search_state.matches.len() {
                                let mp = &search_state.matches[cmi];
                                if mp.line == logical_line {
                                    matches_on_line.iter().position(|&(s, _)| s == mp.column)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let match_fg = Color::Rgb(240, 240, 220);
                        let match_hl = Style::default().bg(theme.match_highlight).fg(match_fg);
                        let current_hl = Style::default().bg(theme.search_highlight).fg(match_fg).add_modifier(Modifier::BOLD);

                        let seg_offset: usize = segments[..seg_idx].iter().map(|s| s.chars().count()).sum();

                        if let Some(ext) = syntax_ext {
                            let full_spans: Vec<(String, Style)> = SyntaxHighlighter::highlight_line(&line_text, ext, theme);
                            let sliced = Self::slice_spans(&full_spans, seg_offset, seg_offset + seg_len);

                            spans = Self::merge_syntax_with_search(
                                &sliced,
                                seg_offset,
                                &matches_on_line,
                                current_match_on_line,
                                theme,
                                match_fg,
                            );
                        } else {
                            let seg_offset_char = seg_offset;
                            let local_matches: Vec<(usize, usize)> = matches_on_line.iter()
                                .filter(|&&(s, e)| s < seg_offset_char + seg_len && e > seg_offset_char)
                                .map(|&(s, e)| (s.saturating_sub(seg_offset_char), e.saturating_sub(seg_offset_char)))
                                .collect();
                            let local_current = current_match_on_line.map(|ci| {
                                if ci < matches_on_line.len() {
                                    let (s, e) = matches_on_line[ci];
                                    if s < seg_offset_char + seg_len && e > seg_offset_char {
                                        Some((s.saturating_sub(seg_offset_char), e.saturating_sub(seg_offset_char)))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }).flatten();

                            let search_spans = Self::build_search_highlighted_spans(
                                segment,
                                0,
                                &local_matches,
                                local_current.and_then(|(s, e)| {
                                    local_matches.iter().position(|&(ms, me)| ms == s && me == e)
                                }),
                                Style::default().fg(theme.fg),
                                match_hl,
                                current_hl,
                            );
                            spans = search_spans;
                        }
                    }

                    let line_style = Style::default().fg(theme.fg).bg(
                        if is_cursor_line { theme.cursor_line } else { theme.bg }
                    );
                    text_lines.push(TextLine::from(spans).style(line_style));
                }
                current_visual += 1;
                if current_visual >= visible_end {
                    break;
                }
            }
            if current_visual >= visible_end {
                break;
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

        // P2.2: Mark engine as rendered (no longer dirty)
        self.dirty.set(false);
    }

    pub fn render_cursor(
        &self,
        frame: &mut Frame,
        area: Rect,
        cursor: &Cursor,
        _theme: &Theme,
        _max_width: usize,
    ) {
        let cursor_line = cursor.position.line;
        if cursor_line < self.scroll_offset.line {
            return;
        }
        let visual_line = cursor_line - self.scroll_offset.line;
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
        if self.word_wrap {
            // approximate: use logical lines for backward compat
            let total = buffer.line_count();
            let start = self.scroll_offset.line.min(total);
            let end = (start + self.viewport_height).min(total);
            start..end
        } else {
            let total = buffer.line_count();
            let start = self.scroll_offset.line.min(total);
            let end = (start + self.viewport_height).min(total);
            start..end
        }
    }

    fn build_search_highlighted_spans(
        line_text: &str,
        col_offset: usize,
        matches_on_line: &[(usize, usize)],
        current_match_on_line: Option<usize>,
        default_style: Style,
        match_style: Style,
        current_match_style: Style,
    ) -> Vec<Span<'static>> {
        if matches_on_line.is_empty() {
            let text = if col_offset < line_text.len() {
                line_text[col_offset..].to_string()
            } else {
                String::new()
            };
            return vec![Span::styled(text, default_style)];
        }

        let mut segments: Vec<(usize, usize)> = Vec::new();
        let mut prev_end = 0usize;
        for &(ms, me) in matches_on_line {
            if ms > prev_end {
                segments.push((prev_end, ms));
            }
            segments.push((ms, me));
            prev_end = me;
        }
        if prev_end < line_text.len() {
            segments.push((prev_end, line_text.len()));
        }

        let mut spans: Vec<Span<'static>> = Vec::new();
        for (seg_start, seg_end) in segments {
            if seg_end <= col_offset {
                continue;
            }
            let text_start = if seg_start < col_offset { col_offset } else { seg_start };
            if text_start >= seg_end {
                continue;
            }
            let text = &line_text[text_start..seg_end];
            let is_match = matches_on_line.iter().any(|&(ms, me)| ms == seg_start && me == seg_end);
            let is_current = if let Some(ci) = current_match_on_line {
                is_match && seg_start == matches_on_line[ci].0 && seg_end == matches_on_line[ci].1
            } else {
                false
            };
            let style = if is_current {
                current_match_style
            } else if is_match {
                match_style
            } else {
                default_style
            };
            spans.push(Span::styled(text.to_string(), style));
        }

        if spans.is_empty() {
            spans.push(Span::styled(" ", default_style));
        }
        spans
    }

    /// P2.4: Replace the O(chars × matches) char-by-char loop with a segment-merge approach.
    /// Merges syntax-highlighted spans with search match intervals in sorted order,
    /// producing spans only at style-transition boundaries (not per-character).
    fn merge_syntax_with_search(
        sliced: &[(String, Style)],
        seg_offset: usize,
        matches_on_line: &[(usize, usize)],
        current_match_on_line: Option<usize>,
        theme: &Theme,
        match_fg: Color,
    ) -> Vec<Span<'static>> {
        if sliced.is_empty() {
            return vec![Span::styled(" ", Style::default().fg(theme.fg))];
        }

        // Compute the absolute end of this segment
        let seg_end = seg_offset + sliced.iter().map(|(t, _)| t.chars().count()).sum::<usize>();

        if matches_on_line.is_empty() {
            // Fast path: no search matches, just return styled syntax spans
            let mut spans = Vec::with_capacity(sliced.len());
            for (t, s) in sliced {
                let merged_fg = s.fg.unwrap_or(theme.fg);
                let merged_bg = s.bg.unwrap_or(theme.bg);
                let mut style = Style::default().fg(merged_fg).bg(merged_bg);
                if s.add_modifier != Modifier::empty() {
                    style = style.add_modifier(s.add_modifier);
                }
                spans.push(Span::styled(t.clone(), style));
            }
            return spans;
        }

        let match_hl = Style::default().bg(theme.match_highlight).fg(match_fg);
        let current_hl = Style::default().bg(theme.search_highlight).fg(match_fg).add_modifier(Modifier::BOLD);

        // Collect all boundaries: syntax segment boundaries + match start/end positions
        let mut boundaries: Vec<usize> = Vec::new();
        let mut pos = seg_offset;
        for (text, _) in sliced {
            boundaries.push(pos);
            pos += text.chars().count();
        }
        boundaries.push(seg_end);

        // Add match boundaries that fall within this segment
        for &(ms, me) in matches_on_line {
            if ms > seg_offset && ms < seg_end {
                boundaries.push(ms);
            }
            if me > seg_offset && me < seg_end {
                boundaries.push(me);
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();

        // Walk through each interval between boundaries
        let mut spans: Vec<Span<'static>> = Vec::new();
        let current_match_range = current_match_on_line
            .filter(|&ci| ci < matches_on_line.len())
            .map(|ci| matches_on_line[ci]);

        for i in 0..boundaries.len() - 1 {
            let start = boundaries[i];
            let end = boundaries[i + 1];
            if start >= end {
                continue;
            }

            // Determine the syntax style at this position by scanning sliced spans
            let synt_style = {
                let mut c = seg_offset;
                let mut style = Style::default().fg(theme.fg);
                for (text, s) in sliced {
                    let t_len = text.chars().count();
                    if start >= c && start < c + t_len {
                        style = *s;
                        break;
                    }
                    c += t_len;
                }
                style
            };

            // Check if this range is within a match interval
            let is_match = matches_on_line.iter().any(|&(ms, me)| start >= ms && end <= me);
            let is_current = current_match_range
                .map(|(cms, cme)| is_match && start >= cms && end <= cme)
                .unwrap_or(false);

            // Extract the text for this interval from the sliced spans
            let text = {
                let mut result = String::new();
                let mut c = seg_offset;
                for (seg_text, _) in sliced {
                    let t_len = seg_text.chars().count();
                    let seg_end2 = c + t_len;
                    if seg_end2 <= start {
                        c = seg_end2;
                        continue;
                    }
                    if c >= end {
                        break;
                    }
                    let local_start = start.saturating_sub(c);
                    let local_end = (end - c).min(t_len);
                    if local_start < t_len && local_end > local_start {
                        let sb = seg_text.char_indices().nth(local_start).map(|(i, _)| i).unwrap_or(seg_text.len());
                        let eb = seg_text.char_indices().nth(local_end).map(|(i, _)| i).unwrap_or(seg_text.len());
                        result.push_str(&seg_text[sb..eb]);
                    }
                    c = seg_end2;
                }
                result
            };

            if text.is_empty() {
                continue;
            }

            let style = if is_current {
                current_hl
            } else if is_match {
                match_hl
            } else {
                let fg = synt_style.fg.unwrap_or(theme.fg);
                let bg = synt_style.bg.unwrap_or(theme.bg);
                let mut style = Style::default().fg(fg).bg(bg);
                if synt_style.add_modifier != Modifier::empty() {
                    style = style.add_modifier(synt_style.add_modifier);
                }
                style
            };

            spans.push(Span::styled(text, style));
        }

        if spans.is_empty() {
            spans.push(Span::styled(" ", Style::default().fg(theme.fg)));
        }
        spans
    }

    pub fn set_viewport(&mut self, height: usize, width: usize) {
        self.viewport_height = height;
        self.viewport_width = width;
    }

    pub fn scroll_to(&mut self, pos: Position, buffer: &Buffer) {
        if self.word_wrap {
            let max_width = self.viewport_width.saturating_sub(2).max(2);
            let total = self.total_visual_lines(buffer, max_width);
            let max_scroll = total.saturating_sub(self.viewport_height);
            let visual_pos = self.logical_to_visual(buffer, pos, max_width);
            let line = visual_pos.line.min(max_scroll);
            self.scroll_offset = Position::new(line, pos.column);
        } else {
            let total = buffer.line_count();
            let max_scroll = total.saturating_sub(self.viewport_height);
            let line = pos.line.min(max_scroll);
            self.scroll_offset = Position::new(line, pos.column);
        }
    }

    pub fn scroll_delta(&mut self, delta_line: isize, buffer: &Buffer) {
        if self.word_wrap {
            let max_width = self.viewport_width.saturating_sub(2).max(2);
            let total = self.total_visual_lines(buffer, max_width);
            let current = self.scroll_offset.line as isize;
            let new_line = (current + delta_line).max(0) as usize;
            let max_scroll = total.saturating_sub(self.viewport_height);
            self.scroll_offset.line = new_line.min(max_scroll);
        } else {
            let total = buffer.line_count();
            let current = self.scroll_offset.line as isize;
            let new_line = (current + delta_line).max(0) as usize;
            let max_scroll = total.saturating_sub(self.viewport_height);
            self.scroll_offset.line = new_line.min(max_scroll);
        }
    }
}
