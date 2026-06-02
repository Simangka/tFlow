use ratatui::text::{Span, Line};
use ratatui::style::{Style, Color, Modifier};
use crate::theme::Theme;

fn vtcolor_to_ratatui(color: vt100::Color, default: Color) -> Color {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Idx(idx) => Color::Indexed(idx),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn vtcell_style(cell: &vt100::Cell, theme: &Theme) -> (Style, bool) {
    let fg = vtcolor_to_ratatui(cell.fgcolor(), theme.fg);
    let bg = vtcolor_to_ratatui(cell.bgcolor(), theme.bg);
    let mut mods = Modifier::empty();
    if cell.bold() { mods |= Modifier::BOLD; }
    if cell.italic() { mods |= Modifier::ITALIC; }
    if cell.underline() { mods |= Modifier::UNDERLINED; }
    (Style::default().fg(fg).bg(bg).add_modifier(mods), cell.inverse())
}

fn cell_text(cell: &vt100::Cell) -> String {
    let raw = cell.contents();
    if raw.is_empty() {
        return " ".to_string();
    }
    raw.chars()
        .map(|c| if c.is_control() && c != '\t' && c != '\n' && c != '\r' { ' ' } else { c })
        .collect()
}

/// Render terminal screen state into ratatui lines.
///
/// Returns (lines, cursor_line, cursor_col) where cursor_line/cursor_col are
/// the 0-based position within the returned lines array (None if hidden or scrolled).
pub fn render_vt100_lines(
    parser: &mut vt100::Parser,
    height: usize,
    scroll_offset: usize,
    theme: &Theme,
    width: usize,
    is_focused: bool,
    exited: bool,
) -> (Vec<Line<'static>>, Option<(usize, u16)>) {
    if height == 0 || width == 0 {
        return (Vec::new(), None);
    }
    parser.set_scrollback(scroll_offset);
    let screen = parser.screen();
    let (rows, cols) = screen.size();
    let rows = rows as usize;

    let pad_top = if height > rows { height - rows } else { 0 };
    let screen_start = if height > rows { 0 } else { rows.saturating_sub(height) };

    let mut lines = Vec::with_capacity(height);
    let bg_fill = Style::default().bg(theme.bg);

    // Pad top with empty lines filled with theme background
    for _ in 0..pad_top {
        let span = Span::styled(" ".repeat(width), bg_fill);
        lines.push(Line::from(span));
    }

    // Render visible screen rows
    for r in screen_start..rows {
        let mut spans: Vec<Span> = Vec::with_capacity(width);
        for c in 0..width.min(cols as usize) {
            if let Some(cell) = screen.cell(r as u16, c as u16) {
                let (style, rev) = vtcell_style(cell, theme);
                let style = if rev {
                    style.add_modifier(Modifier::REVERSED)
                } else {
                    style
                };
                spans.push(Span::styled(cell_text(cell), style));
            } else {
                spans.push(Span::styled(" ", bg_fill));
            }
        }
        while spans.len() < width {
            spans.push(Span::styled(" ", bg_fill));
        }
        lines.push(Line::from(spans));
    }

    // Cursor position (only when not scrolled back, focused, and cursor visible)
    let cursor = if !exited && scroll_offset == 0 && is_focused && !screen.hide_cursor() {
        let (cr, cc) = screen.cursor_position();
        if cr >= screen_start as u16 && cr < rows as u16 {
            let line_idx = pad_top + (cr - screen_start as u16) as usize;
            Some((line_idx, cc))
        } else {
            None
        }
    } else {
        None
    };

    // Apply cursor: swap fg/bg at cursor position
    if let Some((cur_line, cur_col)) = cursor {
        if cur_line < lines.len() {
            let line = &mut lines[cur_line];
            let col = (cur_col as usize).min(width.saturating_sub(1));
            if col < line.spans.len() {
                let span = &line.spans[col];
                // Create reversed style from the span's current style
                let current_fg = span.style.fg.unwrap_or(theme.fg);
                let current_bg = span.style.bg.unwrap_or(theme.bg);
                let cursor_style = Style::default().fg(current_bg).bg(current_fg);
                line.spans[col] = Span::styled(span.content.clone(), cursor_style);
            }
        }
    }

    (lines, cursor)
}

/// Render exit message lines when the shell process has exited.
pub fn render_exit_message(theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let bg = Style::default().bg(theme.bg);
    let msg_style = Style::default().fg(theme.comment).bg(theme.bg);
    let msg = format!(" Process exited. [close tab] ");
    let msg_len = msg.len();

    let mut lines: Vec<Line> = (0..1).map(|_| {
        Line::from(Span::styled(" ".repeat(width), bg))
    }).collect();

    if width > msg_len {
        let pad_left = (width.saturating_sub(msg_len)) / 2;
        let mut spans = vec![Span::styled(" ".repeat(pad_left), bg)];
        spans.push(Span::styled(msg, msg_style));
        let remaining = width.saturating_sub(pad_left + msg_len);
        spans.push(Span::styled(" ".repeat(remaining), bg));
        lines.push(Line::from(spans));
    }

    lines
}
