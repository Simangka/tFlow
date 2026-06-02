use ratatui::{
    Frame,
    layout::{Rect, Alignment},
    widgets::*,
    style::*,
    text::Line as TextLine,
    text::Span,
};

use crate::theme::Theme;

pub struct WidgetRenderer;

impl WidgetRenderer {
    pub fn render_command_bar(
        frame: &mut Frame,
        area: Rect,
        text: &str,
        cursor: usize,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let prompt = ":";
        let display = if text.is_empty() {
            prompt.to_string()
        } else {
            format!("{}{}", prompt, text)
        };

        let style = Style::default()
            .fg(theme.command_bar_fg)
            .bg(theme.command_bar_bg);

        let span = Span::styled(display, style);
        let line = TextLine::from(vec![span]);
        let paragraph = Paragraph::new(vec![line])
            .block(Block::default().style(style));
        frame.render_widget(paragraph, area);

        let cursor_clamped = cursor.min(area.width.saturating_sub(2) as usize);
        let cursor_x = area.x + 1 + cursor_clamped as u16;
        let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(1));
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, area.y));
    }

    pub fn render_search_bar(
        frame: &mut Frame,
        area: Rect,
        query: &str,
        matches: usize,
        current: Option<usize>,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let prompt = "/";
        let match_info = if matches > 0 {
            match current {
                Some(c) => format!(" ({}/{})", c + 1, matches),
                None => format!(" ({} matches)", matches),
            }
        } else if !query.is_empty() {
            " (no matches)".to_string()
        } else {
            String::new()
        };

        let display = format!("{}{}{}", prompt, query, match_info);
        let style = Style::default()
            .fg(theme.command_bar_fg)
            .bg(theme.command_bar_bg);

        let span = Span::styled(display, style);
        let line = TextLine::from(vec![span]);
        let paragraph = Paragraph::new(vec![line])
            .block(Block::default().style(style));
        frame.render_widget(paragraph, area);

        let cursor_clamped = query.len().min(area.width.saturating_sub(2) as usize);
        let cursor_x = area.x + 1 + cursor_clamped as u16;
        let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(1));
        frame.set_cursor_position(ratatui::layout::Position::new(cursor_x, area.y));
    }

    pub fn render_notification(
        frame: &mut Frame,
        area: Rect,
        notification: &crate::core::types::Notification,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let level_color = match notification.level {
            crate::core::types::NotificationLevel::Info => theme.notification_info,
            crate::core::types::NotificationLevel::Warning => theme.notification_warning,
            crate::core::types::NotificationLevel::Error => theme.notification_error,
            crate::core::types::NotificationLevel::Success => theme.notification_success,
        };

        let style = Style::default()
            .fg(level_color)
            .bg(theme.bg);

        let icon = match notification.level {
            crate::core::types::NotificationLevel::Info => "ℹ",
            crate::core::types::NotificationLevel::Warning => "⚠",
            crate::core::types::NotificationLevel::Error => "✖",
            crate::core::types::NotificationLevel::Success => "✔",
        };

        let display = format!(" {} {} ", icon, notification.message);
        let span = Span::styled(display, style.add_modifier(Modifier::BOLD));
        let line = TextLine::from(vec![span]);
        let paragraph = Paragraph::new(vec![line])
            .style(Style::default().bg(theme.bg));
        frame.render_widget(paragraph, area);
    }

    pub fn render_popup(
        frame: &mut Frame,
        area: Rect,
        title: &str,
        content: &str,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg));

        let inner = block.inner(area);
        let max_line_width = inner.width.saturating_sub(2) as usize;
        let max_rows = inner.height as usize;
        let content_lines: Vec<TextLine> = content
            .lines()
            .take(max_rows)
            .map(|l| {
                let truncated: String = if max_line_width > 0 {
                    l.chars().take(max_line_width).collect()
                } else {
                    String::new()
                };
                TextLine::from(vec![Span::styled(
                    truncated,
                    Style::default().fg(theme.fg),
                )])
            })
            .collect();

        let paragraph = Paragraph::new(content_lines)
            .block(block)
            .style(Style::default().bg(theme.bg));

        frame.render_widget(paragraph, area);
    }

    pub fn render_dialog(
        frame: &mut Frame,
        area: Rect,
        title: &str,
        message: &str,
        options: &[&str],
        selected: usize,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_active))
            .style(Style::default().bg(theme.bg));

        let _inner = block.inner(area);

        let mut lines: Vec<TextLine> = Vec::new();

        for msg_line in message.lines() {
            lines.push(TextLine::from(vec![Span::styled(
                msg_line.to_string(),
                Style::default().fg(theme.fg),
            )]));
        }

        lines.push(TextLine::from(vec![Span::styled(
            String::new(),
            Style::default(),
        )]));

        let mut option_spans = Vec::new();
        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                option_spans.push(Span::styled(
                    format!(" [{}] ", opt),
                    Style::default()
                        .fg(theme.bg)
                        .bg(theme.selection_bg)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                option_spans.push(Span::styled(
                    format!(" {} ", opt),
                    Style::default().fg(theme.fg),
                ));
            }
            if i < options.len() - 1 {
                option_spans.push(Span::styled(
                    "  ",
                    Style::default().fg(theme.fg),
                ));
            }
        }
        lines.push(TextLine::from(option_spans));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme.bg));

        frame.render_widget(paragraph, area);
    }

    pub fn render_help_panel(
        frame: &mut Frame,
        area: Rect,
        bindings: &[(String, String)],
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title(" Help ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg));

        let inner = block.inner(area);
        let max_rows = inner.height as usize;

        let mut lines: Vec<TextLine> = Vec::new();

        for (key, desc) in bindings.iter().take(max_rows) {
            let key_span = Span::styled(
                format!(" {:<20} ", key),
                Style::default().fg(theme.statusline_mode),
            );
            let desc_span = Span::styled(
                desc.clone(),
                Style::default().fg(theme.fg),
            );
            lines.push(TextLine::from(vec![key_span, desc_span]));
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme.bg));

        frame.render_widget(paragraph, area);
    }
}
