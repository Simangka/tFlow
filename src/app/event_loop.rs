use crate::app::context::AppContext;
use crate::input::handler::{InputHandler, InputEvent};
use crate::commands::actions::Action;
use crate::core::EditMode;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use ratatui::widgets::{Block, Borders, Paragraph, List, ListItem, Clear, Wrap};
use ratatui::text::{Span, Line};
use ratatui::style::{Style, Modifier};

pub struct EventLoop;

impl EventLoop {
    pub async fn run(config: crate::config::Config) -> Result<(), anyhow::Error> {
        let mut terminal = Self::setup_terminal()?;
        let files = config.files.clone();
        let mut ctx = AppContext::new(config);

        for file in &files {
            let path = std::path::PathBuf::from(file);
            if path.exists() {
                let _ = ctx.open_file(path);
            }
        }

        let mut input_handler = InputHandler::new();
        let _input_handle = input_handler.start_reading();

        loop {
            let event = input_handler.recv().await;
            match event {
                Some(InputEvent::Key(key)) => {
                    if let Err(msg) = Self::handle_key_event(&mut ctx, key) {
                        ctx.push_error(msg);
                    }
                }
                Some(InputEvent::Resize(_cols, _rows)) => {}
                Some(InputEvent::Tick) => {
                    Self::handle_tick(&mut ctx);
                }
                Some(InputEvent::Mouse(mouse)) => {
                    match mouse.kind {
                        crossterm::event::MouseEventKind::ScrollDown => {
                            ctx.handle_action(&Action::MoveDown).ok();
                        }
                        crossterm::event::MouseEventKind::ScrollUp => {
                            ctx.handle_action(&Action::MoveUp).ok();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            if ctx.quit_requested {
                break;
            }

            if let Err(e) = Self::render(&mut terminal, &mut ctx) {
                eprintln!("Render error: {}", e);
                break;
            }
        }

        Self::restore_terminal(&mut terminal)?;
        Ok(())
    }

    fn setup_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, anyhow::Error> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        let _ = crossterm::execute!(stdout, crossterm::event::DisableMouseCapture);
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let mut terminal = ratatui::Terminal::new(backend)?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(terminal)
    }

    fn restore_terminal(terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> Result<(), anyhow::Error> {
        terminal.show_cursor()?;
        crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
        crossterm::terminal::disable_raw_mode()?;
        Ok(())
    }

    fn handle_key_event(ctx: &mut AppContext, raw_key: KeyEvent) -> Result<(), String> {
        let essential_mods = {
            let m = raw_key.modifiers;
            let mut out = KeyModifiers::NONE;
            if m.contains(KeyModifiers::SHIFT) { out |= KeyModifiers::SHIFT; }
            if m.contains(KeyModifiers::CONTROL) { out |= KeyModifiers::CONTROL; }
            if m.contains(KeyModifiers::ALT) { out |= KeyModifiers::ALT; }
            out
        };
        let key = KeyEvent::new(raw_key.code, essential_mods);
        let mode = ctx.editor_mode.mode;

        match mode {
            EditMode::Command => {
                Self::handle_command_mode(ctx, key);
                return Ok(());
            }
            EditMode::Search => {
                Self::handle_search_mode(ctx, key);
                return Ok(());
            }
            EditMode::Insert => {
                match key.code {
                    KeyCode::Esc => {
                        ctx.update_mode(EditMode::Normal);
                        return Ok(());
                    }
                    KeyCode::Enter => {
                        return ctx.handle_action(&Action::InsertNewline);
                    }
                    KeyCode::Backspace => {
                        return ctx.handle_action(&Action::DeleteBackward);
                    }
                    KeyCode::Delete => {
                        return ctx.handle_action(&Action::DeleteForward);
                    }
                    KeyCode::Tab => {
                        return ctx.handle_action(&Action::Indent);
                    }
                    KeyCode::BackTab => {
                        return ctx.handle_action(&Action::Unindent);
                    }
                    KeyCode::Char(c) => {
                        if c == '\n' || c == '\r' {
                            return ctx.handle_action(&Action::InsertNewline);
                        }
                        if c == '\x7f' || c == '\x08' {
                            return ctx.handle_action(&Action::DeleteBackward);
                        }
                        return ctx.handle_action(&Action::InsertChar(c));
                    }
                    _ => {}
                }
            }
            EditMode::Visual | EditMode::VisualLine => {
                match key.code {
                    KeyCode::Esc => {
                        ctx.selection.clear();
                        ctx.update_mode(EditMode::Normal);
                        return Ok(());
                    }
                    _ => {}
                }
            }
            _ => {
                match key.code {
                    KeyCode::Left => return ctx.handle_action(&Action::MoveLeft),
                    KeyCode::Right => return ctx.handle_action(&Action::MoveRight),
                    KeyCode::Up => return ctx.handle_action(&Action::MoveUp),
                    KeyCode::Down => return ctx.handle_action(&Action::MoveDown),
                    KeyCode::Home => return ctx.handle_action(&Action::StartOfLine),
                    KeyCode::End => return ctx.handle_action(&Action::EndOfLine),
                    KeyCode::PageUp => return ctx.handle_action(&Action::PageUp),
                    KeyCode::PageDown => return ctx.handle_action(&Action::PageDown),
                    _ => {}
                }
            }
        }

        if ctx.palette.visible {
            match key.code {
                KeyCode::Esc => {
                    ctx.palette.visible = false;
                    return Ok(());
                }
                KeyCode::Enter => {
                    if let Some(item) = ctx.palette.selected_item() {
                        let action = match &item.action {
                            crate::commands::palette::PaletteAction::Action(a) => a.clone(),
                            crate::commands::palette::PaletteAction::Command(cmd) => Action::ExecuteCommand(cmd.clone()),
                            crate::commands::palette::PaletteAction::File(path) => Action::OpenFileAt(path.clone()),
                            _ => return Ok(()),
                        };
                        ctx.palette.visible = false;
                        return ctx.handle_action(&action);
                    }
                    return Ok(());
                }
                KeyCode::Up => {
                    ctx.palette.select_prev();
                    return Ok(());
                }
                KeyCode::Down => {
                    ctx.palette.select_next();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    ctx.palette.push_char(c);
                    return Ok(());
                }
                KeyCode::Backspace => {
                    ctx.palette.pop_char();
                    return Ok(());
                }
                _ => {}
            }
        }

        if let Some(action) = ctx.keymap.resolve(key, Some(mode)) {
            return ctx.handle_action(&action);
        }

        if mode == EditMode::Insert {
            if let KeyCode::Char(c) = key.code {
                return ctx.handle_action(&Action::InsertChar(c));
            }
        }

        Ok(())
    }

    fn handle_tick(ctx: &mut AppContext) {
        ctx.tick();
    }

    fn render(terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, ctx: &mut AppContext) -> Result<(), anyhow::Error> {
        terminal.draw(|frame| {
            let area = frame.area();
            let layout = ctx.layout.calculate_layout(area);
            let theme = &ctx.theme;
            let buf = &ctx.buffers[ctx.active_buffer];

            let bg_style = Style::default().bg(theme.bg);
            frame.render_widget(ratatui::widgets::Paragraph::new(ratatui::text::Line::from("")).style(bg_style), area);

            ctx.render_engine.render_buffer(
                frame,
                layout.editor,
                buf,
                &ctx.cursor,
                &ctx.selection,
                theme,
                ctx.config.line_numbers.show,
                ctx.config.line_numbers.relative,
            );

            if ctx.cursor.blink_state {
                let cursor_line = ctx.cursor.position.line;
                let cursor_col = ctx.cursor.position.column;
                let total_lines = ctx.active_buffer().line_count();
                let gutter_w = if ctx.config.line_numbers.show {
                    crate::rendering::line_numbers::LineNumbers::gutter_width(total_lines)
                } else {
                    0
                };
                let height = layout.editor.height as usize;
                let scroll_line = ctx.render_engine.scroll_offset.line;
                let visual_line = cursor_line.wrapping_sub(scroll_line);
                if visual_line < height {
                    let text_x = gutter_w + cursor_col as u16;
                    let y = layout.editor.y + visual_line as u16;
                    let text_area_width = layout.editor.width.saturating_sub(gutter_w);
                    if (cursor_col as u16) < text_area_width {
                        frame.set_cursor_position(ratatui::layout::Position::new(layout.editor.x + text_x, y));
                    }
                }
            }

            let mode_str = match ctx.editor_mode.mode {
                EditMode::Normal => "NORMAL",
                EditMode::Insert => "INSERT",
                EditMode::Visual => "VISUAL",
                EditMode::VisualLine => "VISUAL LINE",
                EditMode::Command => "COMMAND",
                EditMode::Search => "SEARCH",
            };

            let info_str = format!(
                " {} | {}:{} | {} lines | {} ",
                mode_str,
                ctx.cursor.position.line + 1,
                ctx.cursor.position.column + 1,
                buf.line_count(),
                buf.name,
            );

            if let Some(statusbar_area) = layout.statusbar {
                let status_style = Style::default()
                    .fg(theme.statusline)
                    .bg(theme.statusline_bg)
                    .add_modifier(Modifier::BOLD);
                let status_bar = Paragraph::new(Line::from(Span::styled(info_str, status_style)))
                    .style(Style::default().bg(theme.statusline_bg));
                frame.render_widget(status_bar, statusbar_area);
            }

            if let Some(cmd_bar_area) = layout.commandbar {
                if ctx.editor_mode.mode == EditMode::Command || ctx.editor_mode.mode == EditMode::Search {
                    let prefix = match ctx.editor_mode.mode {
                        EditMode::Command => ":",
                        EditMode::Search => "/",
                        _ => "",
                    };
                    let buffer = match ctx.editor_mode.mode {
                        EditMode::Command => &ctx.editor_mode.command_buffer,
                        EditMode::Search => &ctx.editor_mode.search_buffer,
                        _ => "",
                    };
                    let cmd_text = format!("{}{}", prefix, buffer);
                    let cmd_style = Style::default().fg(theme.command_bar).bg(theme.bg);
                    let cmd_bar = Paragraph::new(Line::from(Span::styled(cmd_text, cmd_style)))
                        .style(Style::default().bg(theme.bg));
                    frame.render_widget(cmd_bar, cmd_bar_area);
                }
            }

            if let Some(notif_area) = layout.notifications {
                let empty = ctx.notifications.is_empty();
                let notif_style = Style::default().bg(theme.statusline_bg).fg(theme.statusline);
                if empty {
                    frame.render_widget(ratatui::widgets::Paragraph::new(Line::from(Span::styled("", notif_style))).style(notif_style), notif_area);
                } else {
                    let notifications: Vec<Line> = ctx
                        .notifications
                        .iter()
                        .map(|n| {
                            let color = match n.level {
                                crate::core::NotificationLevel::Info => theme.notification_info,
                                crate::core::NotificationLevel::Warning => theme.notification_warning,
                                crate::core::NotificationLevel::Error => theme.notification_error,
                                crate::core::NotificationLevel::Success => theme.notification_success,
                            };
                            let style = Style::default().fg(color).bg(theme.statusline_bg).add_modifier(Modifier::BOLD);
                            Line::from(Span::styled(format!(" {} ", n.message), style))
                        })
                        .collect();
                    let notif_paragraph = Paragraph::new(notifications)
                        .style(Style::default().bg(theme.statusline_bg))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(notif_paragraph, notif_area);
                }
            }

            if let Some(ref ft) = ctx.file_tree {
                if ctx.layout.show_file_tree {
                    if let Some(ft_area) = layout.filetree {
                        let mut ft_items: Vec<ListItem> = ft
                            .entries
                            .iter()
                            .map(|entry| {
                                let indent = "  ".repeat(entry.depth);
                                let icon = if entry.is_dir {
                                    if entry.expanded { "v " } else { "> " }
                                } else {
                                    "  "
                                };
                                let name = format!("{}{}{}", indent, icon, entry.name);
                                ListItem::new(Line::from(Span::styled(
                                    name,
                                    Style::default().fg(theme.fg),
                                )))
                            })
                            .collect();

                        if ft_items.is_empty() {
                            ft_items.push(ListItem::new(Line::from(Span::styled(
                                " (empty) ",
                                Style::default().fg(theme.comment),
                            ))));
                        }

                        let ft_list = List::new(ft_items)
                            .block(Block::default().title(" Files ").borders(Borders::ALL))
                            .highlight_style(Style::default().bg(theme.palette_selection))
                            .highlight_symbol(">>");
                        frame.render_widget(ft_list, ft_area);
                    }
                }
            }

            if ctx.palette.visible {
                if let Some(palette_area) = layout.palette {
                    frame.render_widget(Clear, palette_area);

                    let indices: Vec<usize> = if ctx.palette.query.is_empty() {
                        (0..ctx.palette.items.len()).collect()
                    } else {
                        ctx.palette.filtered.clone()
                    };

                    let items: Vec<ListItem> = indices
                        .iter()
                        .enumerate()
                        .map(|(i, &item_idx)| {
                            let item = &ctx.palette.items[item_idx];
                            let is_selected = i == ctx.palette.selected;
                            let style = if is_selected {
                                Style::default().fg(theme.fg).bg(theme.palette_selection)
                            } else {
                                Style::default().fg(theme.fg).bg(theme.palette)
                            };
                            ListItem::new(Line::from(vec![
                                Span::styled(
                                    format!(" {} ", item.label),
                                    style.add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    format!(" - {}", item.description),
                                    Style::default().fg(theme.comment).bg(style.bg.unwrap_or(theme.palette)),
                                ),
                            ]))
                        })
                        .collect();

                    let palette_list = List::new(items)
                        .block(
                            Block::default()
                                .title(format!(" Palette: {}", ctx.palette.query))
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme.fg)),
                        )
                        .style(Style::default().bg(theme.palette));

                    frame.render_widget(palette_list, palette_area);
                }
            }

            if ctx.layout.show_markdown_preview {
                if let Some(preview_area) = layout.markdown_preview {
                    let text = buf.get_text();
                    let title = if ctx.layout.preview_as_markdown { " Preview (MD) " } else { " Preview " };
                    let rendered = if ctx.layout.preview_as_markdown {
                        crate::markdown::renderer::MarkdownRenderer::render(&text, theme)
                    } else {
                        crate::markdown::plain::PlainTextRenderer::render(&text, theme)
                    };
                    let height = preview_area.height.saturating_sub(2) as usize;
                    let scroll = 0usize;
                    let start = scroll.min(rendered.len().saturating_sub(1));
                    let end = (start + height).min(rendered.len());
                    let visible = if start < end { &rendered[start..end] } else { &[] };
                    let lines: Vec<Line> = visible.iter().map(|l| {
                        Line::from(Span::styled(l.content.clone(), l.style))
                    }).collect();

                    let md_paragraph = Paragraph::new(lines)
                        .block(
                            Block::default()
                                .title(title)
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme.comment)),
                        )
                        .style(Style::default().bg(theme.bg));
                    frame.render_widget(md_paragraph, preview_area);
                }
            }
        })?;
        Ok(())
    }

    fn handle_command_mode(ctx: &mut AppContext, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                ctx.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.buffers.get_mut(ctx.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
            }
            KeyCode::Enter => {
                let cmd = ctx.editor_mode.command_buffer.clone();
                ctx.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.buffers.get_mut(ctx.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
                if !cmd.is_empty() {
                    let action = Action::ExecuteCommand(cmd);
                    let _ = ctx.handle_action(&action);
                }
            }
            KeyCode::Backspace => {
                ctx.editor_mode.command_buffer.pop();
            }
            KeyCode::Char(c) => {
                ctx.editor_mode.command_buffer.push(c);
            }
            _ => {}
        }
    }

    fn handle_search_mode(ctx: &mut AppContext, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                ctx.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.buffers.get_mut(ctx.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
            }
            KeyCode::Enter => {
                let query = ctx.editor_mode.search_buffer.clone();
                ctx.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.buffers.get_mut(ctx.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
                if !query.is_empty() {
                    ctx.search_state.query = query;
                    ctx.search_state.direction = crate::core::SearchDirection::Forward;
                    let _ = ctx.handle_action(&Action::FindNext);
                }
            }
            KeyCode::Backspace => {
                ctx.editor_mode.search_buffer.pop();
            }
            KeyCode::Char(c) => {
                ctx.editor_mode.search_buffer.push(c);
            }
            _ => {}
        }
    }
}
