use ratatui::{
    Frame,
    layout::Rect,
    widgets::Paragraph,
    style::{Style, Modifier, Color},
    text::{Line as TextLine, Span},
};

use crate::theme::Theme;
use crate::editor::modes::EditorMode;
use crate::core::{EditMode, BufferInfo};
use crate::config::Config;

pub struct StatusLine;

impl StatusLine {
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        mode: &EditorMode,
        buffer_info: &BufferInfo,
        _config: &Config,
        theme: &Theme,
        cursor_position: crate::core::Position,
        total_lines: usize,
        is_recording: bool,
        git_branch: Option<&str>,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mode_indicator = Self::mode_indicator(&mode.mode);
        let mode_style = Self::mode_style(&mode.mode, theme);

        let filename = buffer_info.name.as_str();
        let modified = if buffer_info.is_modified { " [+]" } else { "" };
        let readonly = if buffer_info.path.as_ref().map_or(false, |p| {
            p.metadata().ok().map_or(false, |m| m.permissions().readonly())
        }) { " [RO]" } else { "" };

        let line = cursor_position.line + 1;
        let col = cursor_position.column + 1;
        let percentage = if total_lines > 0 {
            ((cursor_position.line as f64 + 1.0) / total_lines as f64 * 100.0) as usize
        } else {
            0
        };

        let encoding = "utf-8";

        let mut all_spans: Vec<Span<'static>> = Vec::new();

        all_spans.push(Span::styled(
            format!(" {} ", mode_indicator),
            mode_style,
        ));

        all_spans.push(Span::styled(
            format!(
                " {} {}{}{} ",
                filename,
                modified,
                readonly,
                if is_recording { " [REC]" } else { "" },
            ),
            Style::default().fg(theme.statusline_filename),
        ));

        let left_len = mode_indicator.len() + 2 + filename.len() + modified.len() + readonly.len()
            + if is_recording { 6 } else { 0 } + 3;

        let right_text = if let Some(branch) = git_branch {
            format!(" {} {}:{} {}% {} {} ", branch, line, col, percentage, encoding, buffer_info.line_count)
        } else {
            format!(" {}:{} {}% {} {} ", line, col, percentage, encoding, buffer_info.line_count)
        };

        let right_len = right_text.len();
        let total_width = area.width as usize;
        let middle_padding = total_width.saturating_sub(left_len + right_len);
        let middle = " ".repeat(middle_padding);

        if !middle.is_empty() {
            all_spans.push(Span::styled(
                middle,
                Style::default().bg(theme.statusline_bg),
            ));
        }

        if let Some(branch) = git_branch {
            all_spans.push(Span::styled(
                format!(" {} ", branch),
                Style::default().fg(theme.statusline_fg),
            ));
        }

        all_spans.push(Span::styled(
            format!(" {}:{} ", line, col),
            Style::default().fg(theme.statusline_fg),
        ));
        all_spans.push(Span::styled(
            format!("{}% ", percentage),
            Style::default().fg(theme.statusline_fg),
        ));
        all_spans.push(Span::styled(
            format!("{} ", encoding),
            Style::default().fg(theme.statusline_fg),
        ));
        all_spans.push(Span::styled(
            format!("{} ", buffer_info.line_count),
            Style::default().fg(theme.statusline_fg),
        ));

        let paragraph = Paragraph::new(TextLine::from(all_spans))
            .style(Style::default().bg(theme.statusline_bg).fg(theme.statusline_fg));
        frame.render_widget(paragraph, area);
    }

    pub fn mode_indicator(mode: &EditMode) -> &'static str {
        match mode {
            EditMode::Normal => "NORMAL",
            EditMode::Insert => "INSERT",
            EditMode::Visual => "VISUAL",
            EditMode::VisualLine => "V-LINE",
            EditMode::Command => "CMD",
            EditMode::Search => "SEARCH",
        }
    }

    pub fn mode_style(mode: &EditMode, theme: &Theme) -> Style {
        let bg = match mode {
            EditMode::Normal => theme.statusline_mode,
            EditMode::Insert => Color::Rgb(60, 180, 100),
            EditMode::Visual | EditMode::VisualLine => Color::Rgb(180, 100, 180),
            EditMode::Command => Color::Rgb(100, 140, 200),
            EditMode::Search => Color::Rgb(200, 160, 60),
        };
        Style::default()
            .fg(match mode {
                EditMode::Normal => Color::Rgb(0, 0, 0),
                EditMode::Insert => Color::Rgb(0, 0, 0),
                EditMode::Visual | EditMode::VisualLine => Color::Rgb(255, 255, 255),
                EditMode::Command => Color::Rgb(255, 255, 255),
                EditMode::Search => Color::Rgb(0, 0, 0),
            })
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    }
}
