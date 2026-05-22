use crate::app::context::AppContext;
use crate::input::handler::{InputHandler, InputEvent};
use crate::commands::actions::Action;
use crate::commands::palette::PaletteMode;
use crate::core::EditMode;
use crate::core::Position;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::workspace::file_tree::TreeDisplayEntry;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
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

        if let Err(e) = Self::render(&mut terminal, &mut ctx) {
            eprintln!("Initial render error: {}", e);
        }

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

        if ctx.layout.show_file_tree && ctx.layout.focused_pane == crate::ui::layout::FocusedPane::FileTree {
            if let Some(ref mut ft) = ctx.file_tree {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        ft.navigate_up();
                        return Ok(());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        ft.navigate_down();
                        return Ok(());
                    }
                    KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                        ft.toggle_expand();
                        if let Some(path) = ft.selected_path() {
                            if path.is_file() {
                                let _ = ctx.open_file(path);
                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        ft.toggle_expand();
                        return Ok(());
                    }
                    KeyCode::Esc => {
                        ctx.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                        return Ok(());
                    }
                    KeyCode::Tab => {
                        ctx.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        if ctx.awaiting_split_key {
            if mode != EditMode::Normal {
                ctx.awaiting_split_key = false;
            } else {
                ctx.awaiting_split_key = false;
                match key.code {
                    KeyCode::Char('v') => return ctx.handle_action(&Action::SplitVertical),
                    KeyCode::Char('s') => return ctx.handle_action(&Action::SplitHorizontal),
                    KeyCode::Char('q') => return ctx.handle_action(&Action::ClosePane),
                    KeyCode::Char('w') => return ctx.handle_action(&Action::NextSplit),
                    KeyCode::Char('h') => return ctx.handle_action(&Action::FocusPaneLeft),
                    KeyCode::Char('j') => return ctx.handle_action(&Action::FocusPaneDown),
                    KeyCode::Char('k') => return ctx.handle_action(&Action::FocusPaneUp),
                    KeyCode::Char('l') => return ctx.handle_action(&Action::FocusPaneRight),
                    _ => {}
                }
            }
        }

        if ctx.layout.focused_pane == crate::ui::layout::FocusedPane::Editor {
            if key.code == KeyCode::Tab && key.modifiers.is_empty() && mode != EditMode::Insert {
                if ctx.layout.show_file_tree {
                    ctx.layout.focused_pane = crate::ui::layout::FocusedPane::FileTree;
                    return Ok(());
                }
            }
        }

        if ctx.palette.visible {
            match key.code {
                KeyCode::Esc => {
                    ctx.palette.visible = false;
                    ctx.layout.show_palette = false;
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
                        ctx.layout.show_palette = false;
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
            _ => {}
        }

        if key.code == KeyCode::Esc && mode == EditMode::Normal && !ctx.search_state.query.is_empty() {
            ctx.search_state = crate::core::SearchState::default();
            return Ok(());
        }

        if key.code == KeyCode::Char('w') && key.modifiers == KeyModifiers::CONTROL && mode == EditMode::Normal {
            ctx.awaiting_split_key = true;
            return Ok(());
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

            let bg_style = Style::default().bg(theme.bg);
            frame.render_widget(ratatui::widgets::Paragraph::new(ratatui::text::Line::from("")).style(bg_style), area);

            let pane_layouts = crate::ui::split::layout_split_node(&ctx.split_manager.root, layout.editor);
            let mut panes_with_rect: Vec<(usize, Rect)> = Vec::new();
            for (id, rect) in pane_layouts {
                panes_with_rect.push((id, rect));
            }
            let editor_is_split = panes_with_rect.len() > 1;

            let mut pane_render_data: Vec<(usize, Rect, Rect, usize, Position, Cursor, Selection)> = Vec::new();
            {
                let mut pane_map: std::collections::HashMap<usize, Rect> = std::collections::HashMap::new();
                for &(id, rect) in &panes_with_rect {
                    pane_map.insert(id, rect);
                }
                ctx.split_manager.for_each_pane(&mut |p| {
                    if let Some(&rect) = pane_map.get(&p.id) {
                        let content_area = if editor_is_split {
                            Rect::new(rect.x + 1, rect.y + 1, rect.width.saturating_sub(2), rect.height.saturating_sub(2))
                        } else {
                            rect
                        };
                        pane_render_data.push((p.id, rect, content_area, p.buffer_id, p.scroll_offset, p.cursor.clone(), p.selection.clone()));
                    }
                });
            }

            for (pane_id, outer_rect, content_rect, buffer_id, pane_scroll, pane_cursor, pane_selection) in pane_render_data.into_iter() {
                let buf = &ctx.buffers[buffer_id];
                let content_height = content_rect.height as usize;
                let content_width = content_rect.width as usize;

                if let Some(pane) = ctx.split_manager.pane_by_id(pane_id) {
                    pane.viewport_height = content_height;
                    pane.viewport_width = content_width;
                }

                ctx.render_engine.set_viewport(content_height, content_width);
                ctx.render_engine.scroll_offset = pane_scroll;

                if editor_is_split {
                    let border_style = if pane_id == ctx.split_manager.active_pane_id {
                        Style::default().fg(theme.border_active)
                    } else {
                        Style::default().fg(theme.comment)
                    };
                    let block = ratatui::widgets::Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .border_style(border_style);
                    frame.render_widget(block, outer_rect);
                }

                let syntax_ext: Option<&str> = buf.path.as_ref()
                    .and_then(|p: &std::path::PathBuf| p.extension())
                    .and_then(|e: &std::ffi::OsStr| e.to_str());
                ctx.render_engine.render_buffer(
                    frame,
                    content_rect,
                    buf,
                    &pane_cursor,
                    &pane_selection,
                    theme,
                    ctx.config.line_numbers.show,
                    ctx.config.line_numbers.relative,
                    syntax_ext,
                    &ctx.search_state,
                );

                if pane_id == ctx.split_manager.active_pane_id && ctx.cursor.blink_state {
                    let cursor_line = pane_cursor.position.line;
                    let cursor_col = pane_cursor.position.column;
                    let total_lines = buf.line_count();
                    let gutter_w = if ctx.config.line_numbers.show {
                        crate::rendering::line_numbers::LineNumbers::gutter_width(total_lines)
                    } else {
                        0
                    };
                    let scroll_line = pane_scroll.line;
                    let visual_line = cursor_line.wrapping_sub(scroll_line);
                    if visual_line < content_height {
                        let text_x = gutter_w + cursor_col as u16;
                        let y = content_rect.y + visual_line as u16;
                        let text_area_width = content_rect.width.saturating_sub(gutter_w);
                        if (cursor_col as u16) < text_area_width {
                            frame.set_cursor_position(ratatui::layout::Position::new(content_rect.x + text_x, y));
                        }
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

            let search_info = if !ctx.search_state.query.is_empty() && !ctx.search_state.matches.is_empty() {
                let total = ctx.search_state.matches.len();
                let current = ctx.search_state.current_match.map(|i| i + 1).unwrap_or(0);
                format!(" [{}/{}]", current, total)
            } else if !ctx.search_state.query.is_empty() {
                " [0/0]".to_string()
            } else {
                String::new()
            };

            let active_buf = &ctx.buffers[ctx.active_buffer];
            let pane_info = if ctx.split_manager.panes_count() > 1 {
                format!(" [panel {} of {}]", ctx.split_manager.active_pane_id + 1, ctx.split_manager.panes_count())
            } else {
                String::new()
            };
            let info_str = format!(
                " {} | {}:{} | {} lines | {}{}{} ",
                mode_str,
                ctx.cursor.position.line + 1,
                ctx.cursor.position.column + 1,
                active_buf.line_count(),
                active_buf.name,
                search_info,
                pane_info,
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

            if ctx.awaiting_split_key && ctx.editor_mode.mode == EditMode::Normal {
                if let Some(cmd_bar_area) = layout.commandbar {
                    let hint_style = Style::default().fg(theme.command_bar).bg(theme.bg).add_modifier(Modifier::BOLD);
                    let hint_text = " Ctrl+w waiting... ";
                    let hint_bar = Paragraph::new(Line::from(Span::styled(hint_text, hint_style)))
                        .style(Style::default().bg(theme.bg));
                    frame.render_widget(hint_bar, cmd_bar_area);
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
                        let display = ft.display_entries();
                        let selected = ft.selected.min(display.len().saturating_sub(1));
                        let scroll = ft.scroll_offset.min(display.len().saturating_sub(1));
                        let height = ft_area.height.saturating_sub(2) as usize;
                        let visible: Vec<&TreeDisplayEntry> = display.iter().skip(scroll).take(height).collect();

                        let border_style = if ctx.layout.focused_pane == crate::ui::layout::FocusedPane::FileTree {
                            Style::default().fg(theme.border_active)
                        } else {
                            Style::default().fg(theme.comment)
                        };

                        let mut items: Vec<ListItem> = visible.iter().enumerate().map(|(vi, de)| {
                            let abs_idx = scroll + vi;
                            let is_selected = abs_idx == selected;
                            let base_style = if is_selected {
                                Style::default().bg(theme.palette_selection).fg(theme.fg).add_modifier(Modifier::BOLD)
                            } else if de.entry.is_dir {
                                Style::default().fg(theme.heading2)
                            } else {
                                Style::default().fg(theme.fg)
                            };
                            let prefix = if de.entry.is_dir {
                                if de.entry.expanded { "[-]" } else { "[+]" }
                            } else {
                                "   "
                            };
                            let line = format!("{}{} {}{}", de.connector, prefix, de.icon, de.display_name);
                            ListItem::new(Line::from(Span::styled(line, base_style)))
                        }).collect();

                        if items.is_empty() {
                            items.push(ListItem::new(Line::from(Span::styled(
                                " (empty) ",
                                Style::default().fg(theme.comment),
                            ))));
                        }

                        let title = if ctx.layout.focused_pane == crate::ui::layout::FocusedPane::FileTree {
                            " Files (focused) "
                        } else {
                            " Files "
                        };

                        let ft_list = List::new(items)
                            .block(Block::default().title(title).borders(Borders::ALL).border_style(border_style))
                            .highlight_style(Style::default().bg(theme.palette_selection));
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
                                    format!(" {}", item.label),
                                    style.add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    format!("  {}", item.description),
                                    Style::default().fg(theme.comment).bg(style.bg.unwrap_or(theme.palette)),
                                ),
                            ]))
                        })
                        .collect();

                    let title = match ctx.palette.mode {
                        PaletteMode::Files => {
                            format!(" Files{} ", if ctx.palette.query.is_empty() { String::new() } else { format!(": {}", ctx.palette.query) })
                        }
                        PaletteMode::Commands => format!(" Commands: {} ", ctx.palette.query),
                        _ => format!(" {} ", ctx.palette.query),
                    };

                    let palette_list = List::new(items)
                        .block(
                            Block::default()
                                .title(title)
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme.border_active)),
                        )
                        .style(Style::default().bg(theme.palette));

                    frame.render_widget(palette_list, palette_area);
                }
            }

            if ctx.layout.show_markdown_preview {
                if let Some(preview_area) = layout.markdown_preview {
                    let rendered = crate::markdown::help::HelpScreen::render(theme);
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
                                .title(" tFlow Help ")
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
                ctx.search_state = crate::core::SearchState::default();
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
