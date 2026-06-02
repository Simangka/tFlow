use ratatui::{
    Frame,
    layout::Rect,
    widgets::*,
    style::*,
    text::Line as TextLine,
    text::Span,
};
use unicode_width::UnicodeWidthChar;

use crate::theme::Theme;
use crate::commands::palette::{CommandPalette, PaletteMode, PaletteItem};

pub struct PanelManager;

impl PanelManager {
    pub fn render_file_tree(
        frame: &mut Frame,
        area: Rect,
        files: &[crate::workspace::FileEntry],
        selected: usize,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title(" Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg));

        let _inner = block.inner(area);
        let mut lines: Vec<TextLine> = Vec::new();

        for (i, entry) in files.iter().enumerate() {
            let indent = "  ".repeat(entry.depth);
            let icon = if entry.is_dir { "[+]" } else { " " };
            let display = format!("{}{} {}", indent, icon, entry.name);

            let style = if i == selected {
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.selection_bg)
            } else {
                Style::default().fg(theme.fg)
            };

            let line = TextLine::from(vec![Span::styled(display, style)]);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme.bg));

        frame.render_widget(paragraph, area);
    }

    pub fn render_command_palette(
        frame: &mut Frame,
        area: Rect,
        palette: &CommandPalette,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title(" Command Palette ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_active))
            .style(Style::default().bg(theme.command_bar_bg));

        let inner = block.inner(area);

        let prompt_style = Style::default()
            .fg(theme.command_bar_fg)
            .bg(theme.command_bar_bg);

        let prompt_line = TextLine::from(vec![
            Span::styled("> ", prompt_style),
            Span::styled(
                palette.query.clone(),
                prompt_style.add_modifier(Modifier::BOLD),
            ),
        ]);

        let separator = TextLine::from(vec![Span::styled(
            "─".repeat(inner.width as usize),
            Style::default().fg(theme.border),
        )]);

        let mut lines = vec![prompt_line, separator];

        let palette_items: Vec<&PaletteItem> = palette
            .filtered
            .iter()
            .filter_map(|&idx| palette.items.get(idx))
            .collect();

        let start_idx = palette.selected.saturating_sub(5);
        if inner.height < 3 {
            return;
        }
        let take = (inner.height as usize).saturating_sub(2);
        let visible_items: Vec<&PaletteItem> = palette_items
            .iter()
            .skip(start_idx)
            .take(take)
            .copied()
            .collect();

        for (relative_idx, item) in visible_items.iter().enumerate() {
            let abs_idx = start_idx + relative_idx;
            let is_selected = abs_idx == palette.selected;

            let mode_is_files = matches!(palette.mode, PaletteMode::Files | PaletteMode::Grep);
            let display = if mode_is_files {
                format!(" {} ", item.label)
            } else {
                format!(" {:20}  {}", item.label, item.description)
            };

            let style = if is_selected {
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };

            let line = TextLine::from(vec![Span::styled(display, style)]);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme.command_bar_bg));

        frame.render_widget(paragraph, area);
    }

    pub fn render_search_results(
        frame: &mut Frame,
        area: Rect,
        results: &[crate::workspace::SearchResult],
        selected: usize,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title(format!(" Search Results ({}) ", results.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg));

        let inner = block.inner(area);
        let mut lines: Vec<TextLine> = Vec::new();

        for (i, result) in results.iter().enumerate() {
            if i >= inner.height as usize {
                break;
            }

            let is_selected = i == selected;
            let file_info = format!(" {}:{} ", result.path.display(), result.line + 1);
            let line_preview = &result.line_content;

            let style = if is_selected {
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.selection_bg)
            } else {
                Style::default().fg(theme.fg)
            };

            let file_style = if is_selected {
                Style::default()
                    .fg(theme.bg)
                    .bg(theme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme.statusline_filename)
                    .add_modifier(Modifier::BOLD)
            };

            let line = TextLine::from(vec![
                Span::styled(file_info, file_style),
                Span::styled(line_preview.clone(), style),
            ]);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme.bg));

        frame.render_widget(paragraph, area);
    }

    pub fn render_tabs(
        frame: &mut Frame,
        area: Rect,
        buffers: &[crate::core::BufferInfo],
        active: usize,
        theme: &Theme,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if buffers.is_empty() {
            frame.render_widget(Block::default(), area);
            return;
        }

        let style = Style::default().bg(theme.tab_bg);

        let mut spans: Vec<Span> = Vec::new();
        let mut remaining_width = area.width as usize;
        let mut separator = true;

        for (i, buf) in buffers.iter().enumerate() {
            if remaining_width < 4 {
                if remaining_width >= 2 {
                    spans.push(Span::styled(
                        "…".to_string(),
                        Style::default().fg(theme.tab_inactive),
                    ));
                }
                break;
            }

            let is_active = i == active;
            let modified = if buf.is_modified { " ●" } else { "" };
            let tab_text = format!(" {} {}{} ", buf.name, modified, if is_active { " " } else { "" });

            let tab_style = if is_active {
                Style::default()
                    .fg(theme.tab_active)
                    .bg(theme.tab_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme.tab_inactive)
                    .bg(theme.tab_bg)
            };

            let tab_width = UnicodeWidthStr_width(&tab_text);
            if tab_width > remaining_width {
                let max_width = remaining_width.saturating_sub(2);
                let mut acc = String::new();
                let mut width_used = 0usize;
                for c in tab_text.chars() {
                    let w = UnicodeWidthChar::width(c).unwrap_or(0);
                    if width_used + w > max_width {
                        break;
                    }
                    acc.push(c);
                    width_used += w;
                }
                spans.push(Span::styled(
                    format!("{}…", acc),
                    tab_style,
                ));
                break;
            }

            if separator && i > 0 {
                spans.push(Span::styled(
                    "│",
                    Style::default().fg(theme.border).bg(theme.tab_bg),
                ));
                remaining_width = remaining_width.saturating_sub(1);
            }

            spans.push(Span::styled(tab_text.clone(), tab_style));
            remaining_width = remaining_width.saturating_sub(tab_width);
            separator = true;
        }

        let line = TextLine::from(spans);
        let paragraph = Paragraph::new(vec![line]).style(style);
        frame.render_widget(paragraph, area);
    }
}

fn UnicodeWidthStr_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    UnicodeWidthStr::width(s)
}
