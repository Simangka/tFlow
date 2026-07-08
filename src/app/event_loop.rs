use crate::app::context::AppContext;
use crate::input::handler::{InputHandler, InputEvent};
use crate::commands::actions::Action;
use crate::commands::palette::PaletteMode;
use crate::core::EditMode;
use crate::core::Position;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::workspace::file_tree::TreeDisplayEntry;
use crate::terminal::renderer::render_vt100_lines;
use crate::lsp::{LspEvent, LspConfig, LanguageServerConfig};
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers, MouseButton};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, List, ListItem, Clear, Wrap};
use ratatui::text::{Span, Line};
use ratatui::style::{Style, Modifier, Color};
use std::time::Duration;


/// Suspend TUI input, hand terminal to a shell, then resume.
/// During the shell the EventStream reader is STOPPED so the child
/// has exclusive access to the host terminal's stdin.
#[allow(dead_code)]
fn suspend_to_external(
    input_handler: &mut InputHandler,
    input_handle: &mut tokio::task::JoinHandle<()>,
) {
    // 1. Kill the old EventStream reader (competing for stdin)
    input_handle.abort();
    // 2. Drop the old channel — the aborted task's tx clone will Err
    //    and the old reader exits cleanly.
    *input_handler = InputHandler::new();
    // 3. Run the shell with exclusive stdin access (blocks)
    crate::terminal::suspend_to_shell();
    // 4. Start a fresh reader on the new channel
    *input_handle = input_handler.start_reading();
}

pub struct EventLoop;

impl EventLoop {
    pub async fn run(config: crate::config::Config) -> Result<(), anyhow::Error> {
        let mut terminal = Self::setup_terminal()?;

        // RAII guard ensures host terminal is restored even on panic
        struct RawModeGuard;
        impl Drop for RawModeGuard {
            fn drop(&mut self) {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
            }
        }
        let _guard = RawModeGuard;

        let mut ctx = AppContext::new(config);

        // Initialize LSP channels and spawn manager task
        let (lsp_cmd_tx, mut lsp_event_rx, lsp_cmd_rx, lsp_event_tx) = crate::lsp::create_lsp_channels();
        ctx.lsp.lsp_tx = Some(lsp_cmd_tx);

        let lsp_lsp_config = LspConfig::default();
        let lsp_lang_config = LanguageServerConfig::default();
        tokio::spawn(async move {
            crate::lsp::run_lsp_manager(lsp_cmd_rx, lsp_event_tx, Some(lsp_lang_config), Some(lsp_lsp_config)).await;
        });

        // Start file system watcher for real-time file tree updates
        ctx.watch_workspace();

        let files = ctx.app.config.files.clone();
        for file in &files {
            let path = std::path::PathBuf::from(file);
            if path.exists() {
                let _ = ctx.open_file(path);
            }
        }

        let mut input_handler = InputHandler::new();
        let mut input_handle = input_handler.start_reading();

        if let Err(e) = Self::render(&mut terminal, &mut ctx) {
            eprintln!("Initial render error: {}", e);
        }

        loop {
            // Poll for both input and LSP events
            let event = input_handler.recv().await;
            match event {
                Some(InputEvent::Key(key)) => {
                    if let Err(msg) = Self::handle_key_event(&mut ctx, key, &mut input_handler, &mut input_handle) {
                        ctx.push_error(msg);
                    }
                    if ctx.editor.editor_mode.mode == EditMode::Insert {
                        ctx.lsp.last_keypress = std::time::Instant::now();
                        // Don't re-trigger completion for nav/accept keys when popup is showing
                        let skip_trigger = (ctx.lsp.show_completion
                            && !ctx.lsp.completion_items.is_empty()
                            && matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Tab))
                            || (!ctx.lsp.show_completion && matches!(key.code, KeyCode::Enter | KeyCode::Tab));
                        if !skip_trigger {
                            ctx.lsp.completion_pending = true;
                        }
                    }
                }
                Some(InputEvent::Resize(_cols, _rows)) => {}
                Some(InputEvent::Tick) => {
                    Self::handle_tick(&mut ctx);
                    // Check completion debounce
                    if ctx.lsp.completion_pending && ctx.lsp.lsp_enabled {
                        let elapsed = ctx.lsp.last_keypress.elapsed();
                        if elapsed >= std::time::Duration::from_millis(80) {
                            ctx.trigger_completion();
                        }
                    }
                }
                Some(InputEvent::Mouse(mouse)) => {
                    // When the terminal pane is in alternate screen mode (opencode,
                    // vim, htop, less, ...) the TUI has its own mouse handling.
                    // Forward the event to the PTY in xterm's default mouse encoding
                    // (CSI M Cb Cx Cy) so the TUI can scroll / click / select natively.
                    // Without this, scrolling in opencode does nothing because we
                    // capture the wheel at the tFlow layer and never send it through.
                    let in_alt_screen = ctx.app.terminal_panel.active()
                        .map(|inst| inst.parser.screen().alternate_screen())
                        .unwrap_or(false);

                    if in_alt_screen && ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::Terminal {
                        let cb: u8 = match mouse.kind {
                            crossterm::event::MouseEventKind::Down(MouseButton::Left) => 0,
                            crossterm::event::MouseEventKind::Down(MouseButton::Middle) => 1,
                            crossterm::event::MouseEventKind::Down(MouseButton::Right) => 2,
                            crossterm::event::MouseEventKind::Up(MouseButton::Left) => 3,
                            crossterm::event::MouseEventKind::Up(MouseButton::Middle) => 4,
                            crossterm::event::MouseEventKind::Up(MouseButton::Right) => 5,
                            crossterm::event::MouseEventKind::Drag(MouseButton::Left) => 32,
                            crossterm::event::MouseEventKind::Drag(MouseButton::Middle) => 33,
                            crossterm::event::MouseEventKind::Drag(MouseButton::Right) => 34,
                            crossterm::event::MouseEventKind::ScrollUp => 64,
                            crossterm::event::MouseEventKind::ScrollDown => 65,
                            crossterm::event::MouseEventKind::ScrollLeft => 66,
                            crossterm::event::MouseEventKind::ScrollRight => 67,
                            _ => 255, // unknown — skip
                        };
                        if cb != 255 {
                            // xterm default encoding: 1-based col/row, each offset by 32.
                            let cx = mouse.column.saturating_add(1).saturating_add(32);
                            let cy = mouse.row.saturating_add(1).saturating_add(32);
                            let mut seq = [0u8; 6];
                            seq[0] = 0x1b; seq[1] = b'['; seq[2] = b'M';
                            seq[3] = cb.saturating_add(32);
                            seq[4] = (cx as u8).min(255);
                            seq[5] = (cy as u8).min(255);
                            ctx.app.terminal_panel.write_active(&seq).ok();
                        }
                    } else {
                        // Normal (non-alt-screen) terminal: use the wheel for
                        // terminal scrollback or editor viewport scroll.
                        match mouse.kind {
                            crossterm::event::MouseEventKind::ScrollDown => {
                                if ctx.app.terminal_panel.visible {
                                    ctx.app.terminal_panel.scroll_down();
                                } else {
                                    // Scroll the editor viewport, don't move the cursor
                                    // (MoveDown would just step the cursor one line).
                                    ctx.handle_action(&Action::PageDown).ok();
                                }
                            }
                            crossterm::event::MouseEventKind::ScrollUp => {
                                if ctx.app.terminal_panel.visible {
                                    ctx.app.terminal_panel.scroll_up();
                                } else {
                                    ctx.handle_action(&Action::PageUp).ok();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some(InputEvent::Paste(data)) => {
                    ctx.handle_paste_event(&data);
                }
                Some(InputEvent::FocusGained) | Some(InputEvent::FocusLost) => {}
                None => {}
            }

            // Drain pending LSP events (non-blocking, batched)
            while let Ok(lsp_event) = lsp_event_rx.try_recv() {
                Self::handle_lsp_event(&mut ctx, lsp_event);
            }

            if ctx.app.quit_requested {
                break;
            }

            // Mark render engine as dirty since state has changed
            ctx.ui.render_engine.dirty.set(true);

            if let Err(e) = Self::render(&mut terminal, &mut ctx) {
                eprintln!("Render error: {}", e);
                break;
            }
        }

        Self::restore_terminal(&mut terminal)?;
        Ok(())
    }

    fn handle_lsp_event(ctx: &mut AppContext, event: LspEvent) {
        match event {
            LspEvent::Diagnostics { doc_id, diagnostics } => {
                if doc_id == ctx.editor.active_buffer {
                    ctx.lsp.lsp_diagnostics = diagnostics;
                }
            }
            LspEvent::CompletionResult { doc_id, items, is_incomplete } => {
                if doc_id == ctx.editor.active_buffer {
                    ctx.push_info(format!("LSP got {} completions", items.len()));
                    // Sort: prefix-exact-match first, then prefix-contains, then others
                    let prefix: String = ctx.editor.buffers.get(ctx.editor.active_buffer)
                        .map(|b| {
                            let line = b.get_line(ctx.editor.cursor.position.line);
                            let col = ctx.editor.cursor.position.column.min(line.len());
                            line[..col].chars().rev()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect::<String>()
                        })
                        .unwrap_or_default()
                        .chars().rev().collect();
                    let mut sorted = items;
                    if !prefix.is_empty() {
                        sorted.sort_by(|a, b| {
                            let a_label = a.insert_text.as_deref().unwrap_or(&a.label).to_lowercase();
                            let b_label = b.insert_text.as_deref().unwrap_or(&b.label).to_lowercase();
                            let a_starts = a_label.starts_with(&prefix);
                            let b_starts = b_label.starts_with(&prefix);
                            let a_contains = a_label.contains(&prefix);
                            let b_contains = b_label.contains(&prefix);
                            match (a_starts, b_starts, a_contains, b_contains) {
                                (true, false, _, _) => std::cmp::Ordering::Less,
                                (false, true, _, _) => std::cmp::Ordering::Greater,
                                (_, _, true, false) => std::cmp::Ordering::Less,
                                (_, _, false, true) => std::cmp::Ordering::Greater,
                                _ => a_label.cmp(&b_label),
                            }
                        });
                    }
                    ctx.lsp.completion_items = sorted;
                    ctx.lsp.show_completion = true;
                    ctx.lsp.completion_selected = 0;
                    let _ = is_incomplete;
                }
            }
            LspEvent::ServerStarted { language, .. } => {
                ctx.push_info(format!("LSP server started: {}", language));
            }
            LspEvent::ServerStopped { language, reason } => {
                ctx.push_info(format!("LSP server stopped: {} ({})", language, reason));
            }
            LspEvent::ServerError { language, error } => {
                ctx.push_error(format!("LSP [{}]: {}", language, error));
            }
            LspEvent::SemanticTokensResult { doc_id, tokens } => {
                if doc_id == ctx.editor.active_buffer {
                    ctx.lsp.lsp_semantic_tokens = tokens;
                }
            }
            _ => {}
        }
    }

    /// Disable Quick Edit Mode on Windows so Ctrl+V arrives as a key event instead
    /// of being intercepted by the console and injected character-by-character.
    /// Neovim and other proper terminal editors all do this.
    #[cfg(windows)]
    fn disable_quick_edit_mode() {
        const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6u32;
        const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
        const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;

        type HANDLE = *mut std::ffi::c_void;
        type BOOL = i32;

        extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> HANDLE;
            fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut u32) -> BOOL;
            fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: u32) -> BOOL;
        }

        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            if handle.is_null() || handle as isize == -1 {
                return;
            }
            let mut mode: u32 = 0;
            if GetConsoleMode(handle, &mut mode) != 0 {
                mode |= ENABLE_EXTENDED_FLAGS;
                mode &= !ENABLE_QUICK_EDIT_MODE;
                SetConsoleMode(handle, mode);
            }
        }
    }

    fn setup_terminal() -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, anyhow::Error> {
        #[cfg(windows)]
        Self::disable_quick_edit_mode();
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        // Enable mouse capture so wheel events come to us (and we can scroll the
        // integrated terminal) instead of falling through to the PTY as raw ANSI
        // sequences — which Windows Terminal happily translates to Up/Down and
        // hands to the running TUI (opencode, vim, etc.) as input history nav.
        let _ = crossterm::execute!(stdout, crossterm::event::EnableMouseCapture);
        // Enable bracketed paste so the terminal sends pasted content as a single
        // event instead of individual key events (which creates a typing animation).
        let _ = crossterm::execute!(stdout, crossterm::event::EnableBracketedPaste);
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let mut terminal = ratatui::Terminal::new(backend)?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(terminal)
    }

    fn restore_terminal(terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> Result<(), anyhow::Error> {
        terminal.show_cursor()?;
        let _ = crossterm::execute!(terminal.backend_mut(), crossterm::event::DisableBracketedPaste);
        crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
        crossterm::terminal::disable_raw_mode()?;
        Ok(())
    }

    fn handle_key_event(
        ctx: &mut AppContext,
        raw_key: KeyEvent,
        _input_handler: &mut InputHandler,
        _input_handle: &mut tokio::task::JoinHandle<()>,
    ) -> Result<(), String> {
        let essential_mods = {
            let m = raw_key.modifiers;
            let mut out = KeyModifiers::NONE;
            if m.contains(KeyModifiers::SHIFT) { out |= KeyModifiers::SHIFT; }
            if m.contains(KeyModifiers::CONTROL) { out |= KeyModifiers::CONTROL; }
            if m.contains(KeyModifiers::ALT) { out |= KeyModifiers::ALT; }
            out
        };
        let key = KeyEvent::new(raw_key.code, essential_mods);
        let mode = ctx.editor.editor_mode.mode;

        if ctx.ui.layout.show_file_tree && ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::FileTree {
            if let Some(ref mut ft) = ctx.ui.file_tree {
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
                        ctx.ui.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                        return Ok(());
                    }
                    KeyCode::Tab => {
                        ctx.ui.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        if ctx.git.branch_view.visible && ctx.ui.layout.focused_pane != crate::ui::layout::FocusedPane::FileTree {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    ctx.git.branch_view.select_prev();
                    return Ok(());
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    ctx.git.branch_view.select_next();
                    return Ok(());
                }
                KeyCode::Enter => {
                    let branch_opt = ctx.git.branch_view.selected_branch();
                    let rp_opt = ctx.git.branch_view.repo_path.clone();
                    if let Some(b) = branch_opt {
                        let file_path = ctx.editor.buffers.get(ctx.editor.active_buffer).and_then(|b| b.path.clone());
                        if let Some(fp) = file_path {
                            match ctx.git.git_manager.checkout_branch(&fp, &b) {
                                Ok(msg) => {
                                    ctx.push_success(msg);
                                    if let Some(buf) = ctx.editor.buffers.get_mut(ctx.editor.active_buffer) {
                                    if !buf.dirty {
                                        let _ = buf.load();
                                    }
                                }
                                if let Some(rp) = rp_opt {
                                        ctx.git.branch_view.refresh(rp);
                                    }
                                }
                                Err(e) => ctx.push_error(e),
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Esc | KeyCode::Tab => {
                    ctx.git.branch_view.visible = false;
                    ctx.ui.layout.show_branch_view = false;
                    return Ok(());
                }
                _ => {}
            }
        }

        if ctx.git.staging_panel.visible && ctx.ui.layout.focused_pane != crate::ui::layout::FocusedPane::FileTree {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    ctx.git.staging_panel.select_prev();
                    return Ok(());
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    ctx.git.staging_panel.select_next();
                    return Ok(());
                }
                KeyCode::Enter => {
                    if let Some(entry) = ctx.git.staging_panel.selected_entry().cloned() {
                        if let crate::git::staging_panel::StagingEntry::File(s) = entry {
                            if s.staged {
                                let _ = ctx.git.git_manager.unstage_file(
                                    &std::env::current_dir().unwrap_or_default().join(&s.path),
                                    &s.path,
                                );
                            } else {
                                let _ = ctx.git.git_manager.stage_file(
                                    &std::env::current_dir().unwrap_or_default().join(&s.path),
                                    &s.path,
                                );
                            }
                            if let Some(buf) = ctx.editor.buffers.get(ctx.editor.active_buffer) {
                                if let Some(ref p) = buf.path {
                                    ctx.git.staging_panel.refresh(&mut ctx.git.git_manager, p);
                                }
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Char(' ') => {
                    if let Some(entry) = ctx.git.staging_panel.selected_entry().cloned() {
                        if let crate::git::staging_panel::StagingEntry::File(s) = entry {
                            ctx.git.staging_panel.toggle_expand(&s.path);
                            if let Some(buf) = ctx.editor.buffers.get(ctx.editor.active_buffer) {
                                if let Some(ref p) = buf.path {
                                    ctx.git.staging_panel.refresh(&mut ctx.git.git_manager, p);
                                }
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Esc | KeyCode::Tab => {
                    ctx.git.staging_panel.visible = false;
                    ctx.ui.layout.show_staging_panel = false;
                    return Ok(());
                }
                _ => {}
            }
        }

        if ctx.app.terminal_panel.visible && ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::Terminal {
            // Ctrl+\ toggles focus back to editor
            if key.code == KeyCode::Char('\x1c') ||
                (key.code == KeyCode::Char('\\') && key.modifiers == KeyModifiers::CONTROL) {
                ctx.app.terminal_panel.unfocus();
                ctx.ui.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                return Ok(());
            }
            match key.code {
                KeyCode::Tab if key.modifiers == KeyModifiers::CONTROL => {
                    ctx.app.terminal_panel.next_instance();
                    return Ok(());
                }
                KeyCode::BackTab => {
                    ctx.app.terminal_panel.prev_instance();
                    return Ok(());
                }
                KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                    ctx.app.terminal_panel.scroll_up();
                    return Ok(());
                }
                KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                    ctx.app.terminal_panel.scroll_down();
                    return Ok(());
                }
                KeyCode::PageUp => {
                    ctx.app.terminal_panel.scroll_up();
                    return Ok(());
                }
                KeyCode::PageDown => {
                    ctx.app.terminal_panel.scroll_down();
                    return Ok(());
                }
                KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                    // Ctrl+U - scroll up half a page
                    let half = (ctx.app.terminal_panel.height as usize / 2).max(1);
                    for _ in 0..half {
                        ctx.app.terminal_panel.scroll_up();
                    }
                    return Ok(());
                }
                KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
                    // Ctrl+D - scroll down half a page
                    let half = (ctx.app.terminal_panel.height as usize / 2).max(1);
                    for _ in 0..half {
                        ctx.app.terminal_panel.scroll_down();
                    }
                    return Ok(());
                }
                KeyCode::F(12) => {
                    // F12 toggles the terminal panel (consistent with the global
                    // binding). Previously this did restart_active() + suspend_to_external()
                    // which killed the running child (opencode, vim, …) and dropped
                    // the user into a host shell — not what the README documents and
                    // not what users expect from a "toggle" key.
                    ctx.app.terminal_panel.toggle();
                    ctx.ui.layout.show_terminal = ctx.app.terminal_panel.visible;
                    if !ctx.app.terminal_panel.visible {
                        ctx.app.terminal_panel.unfocus();
                        ctx.ui.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                    }
                    return Ok(());
                }
                _ => {
                    let input = encode_key(key);

                    // Drain pending output FIRST so ConPTY pipe-break
                    // state changes are visible before we check below.
                    while ctx.app.terminal_panel.drain_all() {
                        // Re-drain until no more data
                    }

                    // If we just processed a pipe-break redraw signal, consume
                    // this keypress (typically Enter) instead of forwarding it
                    // to the shell.  The Enter would otherwise be echoed back
                    // and execute an empty command, producing a double-prompt.
                    // The guard fires within the first 2000ms of the pipe break
                    // (generous enough to survive a tick-first + slow-user-Enter
                    // race, narrow enough that legitimate keystrokes minutes
                    // later are never affected).
                    if let Some(inst) = ctx.app.terminal_panel.active_mut() {
                        let consume = inst.pending_after_pipe_break
                            .map(|t| t.elapsed() < Duration::from_millis(2000))
                            .unwrap_or(false);
                        if consume {
                            inst.pending_after_pipe_break = None;
                            return Ok(());
                        }
                    }

                    // If the terminal session has ended, restart on keypress.
                    if ctx.app.terminal_panel.active()
                        .map_or(false, |inst| !inst.is_running())
                    {
                        ctx.app.terminal_panel.restart_active();
                    }

                    // Forward the key to the PTY.
                    if let Some(inst) = ctx.app.terminal_panel.active() {
                        if let Err(_) = inst.process.write(input.as_bytes()) {
                            // Write failed — the PTY writer may be in a bad state
                            // (e.g. ConPTY closed the pipe after a child process exited).
                            // Try once more with a fresh session.
                            let written = crate::terminal::panel::retry_write(
                                &mut ctx.app.terminal_panel,
                                input.as_bytes(),
                            );
                            if !written {
                                ctx.push_error("Terminal error. Press F12 to restart.");
                            }
                        }
                    }
                    return Ok(());
                }
            }
        }

        // Terminal visible but not focused: still allow scroll keys so the user can
        // inspect output without having to click into the pane first. Only intercept
        // the dedicated scroll keys — everything else falls through to the editor
        // (or wherever the focus actually is).
        if ctx.app.terminal_panel.visible && ctx.ui.layout.focused_pane != crate::ui::layout::FocusedPane::Terminal {
            match key.code {
                KeyCode::PageUp => {
                    ctx.app.terminal_panel.scroll_up();
                    return Ok(());
                }
                KeyCode::PageDown => {
                    ctx.app.terminal_panel.scroll_down();
                    return Ok(());
                }
                KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                    ctx.app.terminal_panel.scroll_up();
                    return Ok(());
                }
                KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                    ctx.app.terminal_panel.scroll_down();
                    return Ok(());
                }
                KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                    let half = (ctx.app.terminal_panel.height as usize / 2).max(1);
                    for _ in 0..half { ctx.app.terminal_panel.scroll_up(); }
                    return Ok(());
                }
                KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
                    let half = (ctx.app.terminal_panel.height as usize / 2).max(1);
                    for _ in 0..half { ctx.app.terminal_panel.scroll_down(); }
                    return Ok(());
                }
                _ => {}
            }
        }

        if ctx.app.awaiting_split_key {
            if mode != EditMode::Normal {
                ctx.app.awaiting_split_key = false;
            } else {
                ctx.app.awaiting_split_key = false;
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

        if ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::Editor {
            if key.code == KeyCode::Tab && key.modifiers.is_empty() && mode != EditMode::Insert {
                if ctx.ui.layout.show_file_tree {
                    ctx.ui.layout.focused_pane = crate::ui::layout::FocusedPane::FileTree;
                    return Ok(());
                }
            }
        }

        if ctx.ui.palette.visible {
            match key.code {
                KeyCode::Esc => {
                    ctx.ui.palette.visible = false;
                    ctx.ui.layout.show_palette = false;
                    return Ok(());
                }
                KeyCode::Enter => {
                    if let Some(item) = ctx.ui.palette.selected_item() {
                        let action = match &item.action {
                            crate::commands::palette::PaletteAction::Action(a) => a.clone(),
                            crate::commands::palette::PaletteAction::Command(cmd) => Action::ExecuteCommand(cmd.clone()),
                            crate::commands::palette::PaletteAction::File(path) => Action::OpenFileAt(path.clone()),
                            _ => return Ok(()),
                        };
                        ctx.ui.palette.visible = false;
                        ctx.ui.layout.show_palette = false;
                        return ctx.handle_action(&action);
                    }
                    return Ok(());
                }
                KeyCode::Up => {
                    ctx.ui.palette.select_prev();
                    return Ok(());
                }
                KeyCode::Down => {
                    ctx.ui.palette.select_next();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    ctx.ui.palette.push_char(c);
                    return Ok(());
                }
                KeyCode::Backspace => {
                    ctx.ui.palette.pop_char();
                    return Ok(());
                }
                _ => {}
            }
        }

        // F12: explicit toggle of the integrated terminal. The keymap has a binding
        // for this too, but we handle it here directly as a safety net so the key
        // always works (F12 is sometimes intercepted by host terminals / Windows
        // accessibility shortcuts before it reaches our keymap).
        if key.code == KeyCode::F(12) {
            ctx.handle_action(&Action::ToggleTerminal).ok();
            return Ok(());
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
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        ctx.update_mode(EditMode::Normal);
                        return Ok(());
                    }
                    KeyCode::Up if ctx.lsp.show_completion && !ctx.lsp.completion_items.is_empty() => {
                        ctx.lsp.completion_selected = ctx.lsp.completion_selected.saturating_sub(1);
                        return Ok(());
                    }
                    KeyCode::Down if ctx.lsp.show_completion && !ctx.lsp.completion_items.is_empty() => {
                        let max = ctx.lsp.completion_items.len().saturating_sub(1);
                        ctx.lsp.completion_selected = (ctx.lsp.completion_selected + 1).min(max);
                        return Ok(());
                    }
                    KeyCode::Enter if ctx.lsp.show_completion && !ctx.lsp.completion_items.is_empty() => {
                        ctx.accept_completion();
                        return Ok(());
                    }
                    KeyCode::Tab if ctx.lsp.show_completion && !ctx.lsp.completion_items.is_empty() => {
                        ctx.accept_completion();
                        return Ok(());
                    }
                    KeyCode::Enter => {
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        return ctx.handle_action(&Action::InsertNewline);
                    }
                    KeyCode::Backspace => {
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        return ctx.handle_action(&Action::DeleteBackward);
                    }
                    KeyCode::Delete => {
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        return ctx.handle_action(&Action::DeleteForward);
                    }
                    KeyCode::Tab => {
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        return ctx.handle_action(&Action::Indent);
                    }
                    KeyCode::BackTab => {
                        return ctx.handle_action(&Action::Unindent);
                    }
                    KeyCode::Char('v') if key.modifiers == KeyModifiers::CONTROL => {
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        return ctx.handle_action(&Action::Paste);
                    }
                    KeyCode::Char('p') if key.modifiers == KeyModifiers::NONE => {
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        return ctx.handle_action(&Action::Paste);
                    }
                    KeyCode::Char(c) => {
                        if c == '\n' || c == '\r' {
                            return ctx.handle_action(&Action::InsertNewline);
                        }
                        if c == '\x7f' || c == '\x08' {
                            return ctx.handle_action(&Action::DeleteBackward);
                        }
                        ctx.lsp.show_completion = false;
                        ctx.lsp.completion_items.clear();
                        return ctx.handle_action(&Action::InsertChar(c));
                    }
                    _ => {}
                }
            }
            EditMode::Visual | EditMode::VisualLine => {
                match key.code {
                    KeyCode::Esc => {
                        ctx.editor.selection.clear();
                        ctx.update_mode(EditMode::Normal);
                        return Ok(());
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        if key.code == KeyCode::Esc && mode == EditMode::Normal && !ctx.editor.search_state.query.is_empty() {
            ctx.editor.search_state = crate::core::SearchState::default();
            return Ok(());
        }

        if key.code == KeyCode::Char('w') && key.modifiers == KeyModifiers::CONTROL && mode == EditMode::Normal {
            ctx.app.awaiting_split_key = true;
            return Ok(());
        }

        if let Some(action) = ctx.app.keymap.resolve(key, Some(mode)) {
            return ctx.handle_action(&action);
        }

        if mode == EditMode::Insert {
            if let KeyCode::Char(c) = key.code {
                return ctx.handle_action(&Action::InsertChar(c));
            }
        }

        Ok(())
    }

    fn process_input_event(
        ctx: &mut AppContext,
        event: InputEvent,
        input_handler: &mut InputHandler,
        input_handle: &mut tokio::task::JoinHandle<()>,
    ) {
        match event {
            InputEvent::Key(key) => {
                if let Err(msg) = Self::handle_key_event(ctx, key, input_handler, input_handle) {
                    ctx.push_error(msg);
                }
                if ctx.editor.editor_mode.mode == EditMode::Insert {
                    ctx.lsp.last_keypress = std::time::Instant::now();
                    let skip_trigger = (ctx.lsp.show_completion
                        && !ctx.lsp.completion_items.is_empty()
                        && matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Enter | KeyCode::Tab))
                        || (!ctx.lsp.show_completion && matches!(key.code, KeyCode::Enter | KeyCode::Tab));
                    if !skip_trigger {
                        ctx.lsp.completion_pending = true;
                    }
                }
            }
            InputEvent::Resize(_cols, _rows) => {}
            InputEvent::Tick => {
                Self::handle_tick(ctx);
            }
            InputEvent::Paste(data) => {
                ctx.handle_paste_event(&data);
            }
            InputEvent::Mouse(mouse) => {
                let in_alt_screen = ctx.app.terminal_panel.active()
                    .map(|inst| inst.parser.screen().alternate_screen())
                    .unwrap_or(false);

                if in_alt_screen && ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::Terminal {
                    let cb: u8 = match mouse.kind {
                        crossterm::event::MouseEventKind::Down(MouseButton::Left) => 0,
                        crossterm::event::MouseEventKind::Down(MouseButton::Middle) => 1,
                        crossterm::event::MouseEventKind::Down(MouseButton::Right) => 2,
                        crossterm::event::MouseEventKind::Up(MouseButton::Left) => 3,
                        crossterm::event::MouseEventKind::Up(MouseButton::Middle) => 4,
                        crossterm::event::MouseEventKind::Up(MouseButton::Right) => 5,
                        crossterm::event::MouseEventKind::Drag(MouseButton::Left) => 32,
                        crossterm::event::MouseEventKind::Drag(MouseButton::Middle) => 33,
                        crossterm::event::MouseEventKind::Drag(MouseButton::Right) => 34,
                        crossterm::event::MouseEventKind::ScrollUp => 64,
                        crossterm::event::MouseEventKind::ScrollDown => 65,
                        crossterm::event::MouseEventKind::ScrollLeft => 66,
                        crossterm::event::MouseEventKind::ScrollRight => 67,
                        _ => 255,
                    };
                    if cb != 255 {
                        let cx = mouse.column.saturating_add(1).saturating_add(32);
                        let cy = mouse.row.saturating_add(1).saturating_add(32);
                        let mut seq = [0u8; 6];
                        seq[0] = 0x1b; seq[1] = b'['; seq[2] = b'M';
                        seq[3] = cb.saturating_add(32);
                        seq[4] = (cx as u8).min(255);
                        seq[5] = (cy as u8).min(255);
                        ctx.app.terminal_panel.write_active(&seq).ok();
                    }
                } else {
                    match mouse.kind {
                        crossterm::event::MouseEventKind::ScrollDown => {
                            if ctx.app.terminal_panel.visible {
                                ctx.app.terminal_panel.scroll_down();
                            } else {
                                ctx.handle_action(&Action::PageDown).ok();
                            }
                        }
                        crossterm::event::MouseEventKind::ScrollUp => {
                            if ctx.app.terminal_panel.visible {
                                ctx.app.terminal_panel.scroll_up();
                            } else {
                                ctx.handle_action(&Action::PageUp).ok();
                            }
                        }
                        _ => {}
                    }
                }
            }
            InputEvent::FocusGained | InputEvent::FocusLost => {}
        }
    }

    fn handle_tick(ctx: &mut AppContext) {
        ctx.tick();
        if ctx.app.terminal_panel.visible {
            while ctx.app.terminal_panel.drain_all() {
                // Re-drain until no more data
            }

            // NOTE: a DSR (\x1b[5n) nudge was previously sent here as a
            // Windows ConPTY freeze workaround, but the trailing 'n' leaks
            // through to non-alt-screen TUIs (ollama, etc.) and appears as
            // unwanted input.  The other mechanisms — Err polling in the
            // reader, drain-on-tick, consume-guard — handle the freeze
            // acceptably without the DSR side-effect.
        }
    }

    fn render(terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, ctx: &mut AppContext) -> Result<(), anyhow::Error> {
        if !ctx.ui.render_engine.needs_render() {
            return Ok(());
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let layout = ctx.ui.layout.calculate_layout(area);
            let theme = &ctx.ui.theme;

            let bg_style = Style::default().bg(theme.bg);
            frame.render_widget(ratatui::widgets::Paragraph::new(ratatui::text::Line::from("")).style(bg_style), area);

            let pane_layouts = crate::ui::split::layout_split_node(&ctx.app.split_manager.root, layout.editor);
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
                ctx.app.split_manager.for_each_pane(&mut |p| {
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
                let buf = &ctx.editor.buffers[buffer_id];
                let content_height = content_rect.height as usize;
                let content_width = content_rect.width as usize;

                if let Some(pane) = ctx.app.split_manager.pane_by_id(pane_id) {
                    pane.viewport_height = content_height;
                    pane.viewport_width = content_width;
                }

                ctx.ui.render_engine.set_viewport(content_height, content_width);
                ctx.ui.render_engine.scroll_offset = pane_scroll;

                if editor_is_split {
                    let border_style = if pane_id == ctx.app.split_manager.active_pane_id {
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
                let file_blame: Option<Vec<Option<crate::git::BlameInfo>>> = if ctx.git.show_blame {
                    buf.path.as_ref().and_then(|p| {
                        ctx.git.git_manager.get_blame(p).map(|b| {
                            let max_line = buf.line_count();
                            (0..max_line).map(|i| {
                                b.iter().find(|bl| bl.line == i).cloned()
                            }).collect()
                        })
                    })
                } else { None };
                let blame_ref = file_blame.as_ref().map(|v| &v[..]);
                ctx.ui.render_engine.render_buffer(
                    frame,
                    content_rect,
                    buf,
                    &pane_cursor,
                    &pane_selection,
                    theme,
                    ctx.app.config.line_numbers.show,
                    ctx.app.config.line_numbers.relative,
                    syntax_ext,
                    &ctx.editor.search_state,
                    blame_ref,
                );

                if pane_id == ctx.app.split_manager.active_pane_id && ctx.editor.cursor.blink_state {
                    let cursor_line = pane_cursor.position.line;
                    let cursor_col = pane_cursor.position.column;
                    let total_lines = buf.line_count();
                    let gutter_w = if ctx.app.config.line_numbers.show {
                        crate::rendering::line_numbers::LineNumbers::gutter_width(total_lines)
                    } else {
                        0
                    };
                    let blame_w = if ctx.git.show_blame {
                        if let Some(ref bd) = file_blame {
                            crate::rendering::blame_gutter::compute_width(bd)
                        } else { 0 }
                    } else { 0 };
                    let text_area_width = content_rect.width.saturating_sub(gutter_w + blame_w);
                    let (vis_cursor_line, vis_cursor_col) = if ctx.app.config.editor.word_wrap {
                        let vpos = ctx.ui.render_engine.logical_to_visual(
                            buf,
                            Position::new(cursor_line, cursor_col),
                            text_area_width as usize,
                        );
                        (vpos.line, vpos.column)
                    } else {
                        (cursor_line, cursor_col)
                    };
                    let scroll_line = pane_scroll.line;
                    let visual_line = vis_cursor_line.wrapping_sub(scroll_line);
                    if visual_line < content_height {
                        let text_x = gutter_w + blame_w + vis_cursor_col as u16;
                        let y = content_rect.y + visual_line as u16;
                        if (vis_cursor_col as u16) < text_area_width {
                            frame.set_cursor_position(ratatui::layout::Position::new(content_rect.x + text_x, y));
                        }
                    }
                }
            }

            let mode_str = match ctx.editor.editor_mode.mode {
                EditMode::Normal => "NORMAL",
                EditMode::Insert => "INSERT",
                EditMode::Visual => "VISUAL",
                EditMode::VisualLine => "VISUAL LINE",
                EditMode::Command => "COMMAND",
                EditMode::Search => "SEARCH",
            };

            let search_info = if !ctx.editor.search_state.query.is_empty() && !ctx.editor.search_state.matches.is_empty() {
                let total = ctx.editor.search_state.matches.len();
                let current = ctx.editor.search_state.current_match.map(|i| i + 1).unwrap_or(0);
                format!(" [{}/{}]", current, total)
            } else if !ctx.editor.search_state.query.is_empty() {
                " [0/0]".to_string()
            } else {
                String::new()
            };

            let active_buf = &ctx.editor.buffers[ctx.editor.active_buffer];
            let pane_info = if ctx.app.split_manager.panes_count() > 1 {
                format!(" [panel {} of {}]", ctx.app.split_manager.active_pane_id + 1, ctx.app.split_manager.panes_count())
            } else {
                String::new()
            };
            let diag_str = if !ctx.lsp.lsp_diagnostics.is_empty() {
                let errs = ctx.lsp.lsp_diagnostics.iter().filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR)).count();
                let warns = ctx.lsp.lsp_diagnostics.iter().filter(|d| d.severity == Some(lsp_types::DiagnosticSeverity::WARNING)).count();
                if errs > 0 && warns > 0 {
                    format!(" {}E {}W ", errs, warns)
                } else if errs > 0 {
                    format!(" {}E ", errs)
                } else if warns > 0 {
                    format!(" {}W ", warns)
                } else {
                    format!(" {}D ", ctx.lsp.lsp_diagnostics.len())
                }
            } else {
                String::new()
            };
            let info_str = format!(
                " {} | {}:{} | {} lines | {}{}{}{} ",
                mode_str,
                ctx.editor.cursor.position.line + 1,
                ctx.editor.cursor.position.column + 1,
                active_buf.line_count(),
                active_buf.name,
                search_info,
                diag_str,
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
                if ctx.editor.editor_mode.mode == EditMode::Command || ctx.editor.editor_mode.mode == EditMode::Search {
                    let prefix = match ctx.editor.editor_mode.mode {
                        EditMode::Command => ":",
                        EditMode::Search => "/",
                        _ => "",
                    };
                    let buffer = match ctx.editor.editor_mode.mode {
                        EditMode::Command => &ctx.editor.editor_mode.command_buffer,
                        EditMode::Search => &ctx.editor.editor_mode.search_buffer,
                        _ => "",
                    };
                    let cmd_text = format!("{}{}", prefix, buffer);
                    let cmd_style = Style::default().fg(theme.command_bar).bg(theme.bg);
                    let cmd_bar = Paragraph::new(Line::from(Span::styled(cmd_text, cmd_style)))
                        .style(Style::default().bg(theme.bg));
                    frame.render_widget(cmd_bar, cmd_bar_area);
                }
            }

            if ctx.app.awaiting_split_key && ctx.editor.editor_mode.mode == EditMode::Normal {
                if let Some(cmd_bar_area) = layout.commandbar {
                    let hint_style = Style::default().fg(theme.command_bar).bg(theme.bg).add_modifier(Modifier::BOLD);
                    let hint_text = " Ctrl+w waiting... ";
                    let hint_bar = Paragraph::new(Line::from(Span::styled(hint_text, hint_style)))
                        .style(Style::default().bg(theme.bg));
                    frame.render_widget(hint_bar, cmd_bar_area);
                }
            }

            if let Some(notif_area) = layout.notifications {
                let empty = ctx.ui.notifications.is_empty();
                let notif_style = Style::default().bg(theme.statusline_bg).fg(theme.statusline);
                if empty {
                    frame.render_widget(ratatui::widgets::Paragraph::new(Line::from(Span::styled("", notif_style))).style(notif_style), notif_area);
                } else {
                    let notifications: Vec<Line> = ctx.ui
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

            if let Some(ref ft) = ctx.ui.file_tree {
                if ctx.ui.layout.show_file_tree {
                    if let Some(ft_area) = layout.filetree {
                        let display = ft.display_entries();
                        let selected = ft.selected.min(display.len().saturating_sub(1));
                        let scroll = ft.scroll_offset.min(display.len().saturating_sub(1));
                        let height = ft_area.height.saturating_sub(2) as usize;
                        let visible: Vec<&TreeDisplayEntry> = display.iter().skip(scroll).take(height).collect();

                        let border_style = if ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::FileTree {
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

                        let title = if ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::FileTree {
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

            if ctx.git.staging_panel.visible {
                if let Some(staging_area) = layout.staging_panel {
                    frame.render_widget(Clear, staging_area);

                    let entries = &ctx.git.staging_panel.data;
                    let items: Vec<ListItem> = entries.iter().enumerate().map(|(i, entry)| {
                        let is_selected = i == ctx.git.staging_panel.selected;
                        let (label, style): (String, Style) = match entry {
                            crate::git::staging_panel::StagingEntry::File(s) => {
                                let prefix = if s.staged { "+".to_string() } else { format!("{}", s.status) };
                                let base = if is_selected {
                                    Style::default().fg(theme.fg).bg(theme.palette_selection)
                                } else {
                                    Style::default().fg(theme.fg).bg(theme.bg)
                                };
                                (format!(" {} {}", prefix, s.path), base)
                            }
                            crate::git::staging_panel::StagingEntry::Hunk { ref hunk, .. } => {
                                let header = hunk.header.trim().chars().take(40).collect::<String>();
                                let base = if is_selected {
                                    Style::default().fg(theme.comment).bg(theme.palette_selection)
                                } else {
                                    Style::default().fg(theme.comment).bg(theme.bg)
                                };
                                (format!("   @ {} {} {}", hunk.old_start, hunk.new_start, header), base)
                            }
                        };
                        ListItem::new(Line::from(Span::styled(label, style)))
                    }).collect();

                    let stage_list = List::new(items)
                        .block(
                            Block::default()
                                .title(" Staging ")
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme.border_active)),
                        )
                        .style(Style::default().bg(theme.bg));
                    frame.render_widget(stage_list, staging_area);
                }
            }

            if ctx.git.branch_view.visible {
                if let Some(bv_area) = layout.branch_view {
                    frame.render_widget(Clear, bv_area);

                    let items: Vec<ListItem> = ctx.git.branch_view.data.iter().enumerate().map(|(i, entry)| {
                        let is_selected = i == ctx.git.branch_view.selected;
                        let style = if is_selected {
                            Style::default().fg(theme.fg).bg(theme.palette_selection)
                        } else if entry.is_head {
                            Style::default().fg(theme.keyword).bg(theme.bg)
                        } else {
                            Style::default().fg(theme.fg).bg(theme.bg)
                        };
                        ListItem::new(Line::from(Span::styled(&entry.text, style)))
                    }).collect();

                    let list = List::new(items)
                        .block(
                            Block::default()
                                .title(" Branches ")
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme.border_active)),
                        )
                        .style(Style::default().bg(theme.bg));
                    frame.render_widget(list, bv_area);
                }
            }

            if ctx.app.terminal_panel.visible {
                if let Some(term_area) = layout.terminal {
                    frame.render_widget(Clear, term_area);

                    let term_instances = ctx.app.terminal_panel.instances.len();
                    let active_idx = ctx.app.terminal_panel.active_idx;

                    let is_focused = ctx.ui.layout.focused_pane == crate::ui::layout::FocusedPane::Terminal;
                    let border_fg = if is_focused { theme.border_active } else { theme.border };
                    let bg_style = Style::default().bg(theme.bg);

                    // Fill entire terminal area with theme background
                    frame.render_widget(ratatui::widgets::Block::default().style(bg_style), term_area);

                    // Tab bar
                    let tab_height = 1u16;
                    let tab_area = Rect::new(term_area.x, term_area.y, term_area.width, tab_height);
                    let output_area = Rect::new(term_area.x, term_area.y + tab_height, term_area.width, term_area.height.saturating_sub(tab_height + 1));

                    // Status line
                    let status_area = Rect::new(term_area.x, term_area.y + term_area.height.saturating_sub(1), term_area.width, 1);

                    // Resize PTY+grid to match the display area (excl. borders)
                    let resize_cols = output_area.width.saturating_sub(2).max(10);
                    let resize_rows = output_area.height.max(1);
                    ctx.app.terminal_panel.resize_active(resize_cols, resize_rows);

                    while ctx.app.terminal_panel.drain_all() {
                        // Re-drain until no more data
                    }

                    // Render tab bar
                    let mut tab_spans: Vec<Span> = Vec::new();
                    for i in 0..term_instances {
                        let is_active = i == active_idx;
                        let tab_style = if is_active {
                            Style::default().fg(theme.fg).bg(theme.palette_selection).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.comment).bg(theme.bg)
                        };
                        let label = ctx.app.terminal_panel.instances[i].title.as_str();
                        tab_spans.push(Span::styled(format!(" {} ", label), tab_style));
                    }
                    let tab_bg = Line::from(Span::styled(" ".repeat(tab_area.width as usize), bg_style));
                    frame.render_widget(tab_bg, tab_area);
                    let tab_line = Line::from(tab_spans);
                    frame.render_widget(tab_line, tab_area);

                    // Render terminal output
                    let output_height = output_area.height as usize;
                    let output_width = output_area.width as usize;
                    let content_width = output_width.saturating_sub(2); // account for borders

                    let terminal_exited = ctx.app.terminal_panel.instances.get(active_idx)
                        .map(|inst| !inst.is_running())
                        .unwrap_or(false);
                    if let Some(inst) = ctx.app.terminal_panel.instances.get_mut(active_idx) {
                        let (lines, _) = render_vt100_lines(
                            &mut inst.parser,
                            output_height,
                            inst.scroll_offset,
                            &ctx.ui.theme,
                            content_width,
                            is_focused,
                            terminal_exited,
                        );

                        let para = Paragraph::new(lines)
                            .block(
                                Block::default()
                                    .borders(Borders::LEFT | Borders::RIGHT)
                                    .border_style(Style::default().fg(border_fg)),
                            )
                            .style(Style::default().bg(ctx.ui.theme.bg));
                        frame.render_widget(para, output_area);

                        // Exit overlay
                        if let Some(msg) = inst.exit_message() {
                            let overlay_style = Style::default()
                                .fg(theme.bg)
                                .bg(Color::Red);
                            let exit_line = Line::from(Span::styled(msg, overlay_style));
                            let exit_para = Paragraph::new(exit_line)
                                .style(Style::default().bg(Color::Red));
                            let exit_area = Rect::new(
                                output_area.x + 1,
                                output_area.y + output_area.height.saturating_sub(2),
                                (msg.len() as u16).min(output_area.width.saturating_sub(2)),
                                1,
                            );
                            frame.render_widget(ratatui::widgets::Clear, exit_area);
                            frame.render_widget(exit_para, exit_area);
                        }
                    }

                    // Render status line
                    let (shell_name, scroll_info) = ctx.app.terminal_panel.instances.get_mut(active_idx)
                        .map(|i| {
                            let info = if i.scroll_offset > 0 { format!(" [+{}]", i.scroll_offset) } else { String::new() };
                            (i.shell.as_str(), info)
                        })
                        .unwrap_or(("term", String::new()));
                    let status_text = format!(" {} {} | {}x{} {}",
                        if is_focused { ">" } else { " " },
                        shell_name,
                        output_width, output_height,
                        scroll_info,
                    );
                    let status_style = if is_focused {
                        Style::default().fg(theme.fg).bg(theme.statusline_bg)
                    } else {
                        Style::default().fg(theme.comment).bg(theme.bg)
                    };
                    let status_bg = Line::from(Span::styled(" ".repeat(status_area.width as usize), status_style));
                    frame.render_widget(status_bg, status_area);
                    frame.render_widget(
                        Line::from(Span::styled(status_text, status_style)),
                        status_area,
                    );
                }
            }

            if ctx.ui.palette.visible {
                if let Some(palette_area) = layout.palette {
                    frame.render_widget(Clear, palette_area);

                    let indices: Vec<usize> = if ctx.ui.palette.query.is_empty() {
                        (0..ctx.ui.palette.items.len()).collect()
                    } else {
                        ctx.ui.palette.filtered.clone()
                    };

                    let items: Vec<ListItem> = indices
                        .iter()
                        .enumerate()
                        .map(|(i, &item_idx)| {
                            let item = &ctx.ui.palette.items[item_idx];
                            let is_selected = i == ctx.ui.palette.selected;
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

                    let title = match ctx.ui.palette.mode {
                        PaletteMode::Files => {
                            format!(" Files{} ", if ctx.ui.palette.query.is_empty() { String::new() } else { format!(": {}", ctx.ui.palette.query) })
                        }
                        PaletteMode::Commands => format!(" Commands: {} ", ctx.ui.palette.query),
                        _ => format!(" {} ", ctx.ui.palette.query),
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

            // Completion popup overlay
            if ctx.lsp.show_completion && !ctx.lsp.completion_items.is_empty() {
                let max_h = 10u16.min(ctx.lsp.completion_items.len() as u16);
                let max_w = ctx.lsp.completion_items.iter()
                    .map(|i| i.label.len() as u16 + 6)
                    .max()
                    .unwrap_or(20)
                    .min(60);
                let popup_x = layout.editor.x + 2;
                let popup_y = layout.editor.y + 2;
                let popup_w = max_w.min(frame.area().width.saturating_sub(popup_x));
                let popup_rect = Rect::new(
                    popup_x.min(frame.area().width.saturating_sub(popup_w)),
                    popup_y.min(frame.area().height.saturating_sub(max_h + 2)),
                    popup_w,
                    max_h + 2,
                );
                frame.render_widget(Clear, popup_rect);
                let items: Vec<ListItem> = ctx.lsp.completion_items.iter().enumerate().map(|(i, item)| {
                    let is_selected = i == ctx.lsp.completion_selected;
                    let style = if is_selected {
                        Style::default().fg(ctx.ui.theme.bg).bg(ctx.ui.theme.keyword)
                    } else {
                        Style::default().fg(ctx.ui.theme.fg).bg(ctx.ui.theme.cursor_line)
                    };
                    let kind_str = match item.kind {
                        Some(lsp_types::CompletionItemKind::METHOD | lsp_types::CompletionItemKind::FUNCTION) => "fn",
                        Some(lsp_types::CompletionItemKind::CLASS | lsp_types::CompletionItemKind::STRUCT) => "cl",
                        Some(lsp_types::CompletionItemKind::MODULE) => "md",
                        Some(lsp_types::CompletionItemKind::KEYWORD) => "kw",
                        Some(lsp_types::CompletionItemKind::VARIABLE | lsp_types::CompletionItemKind::FIELD) => "va",
                        Some(lsp_types::CompletionItemKind::SNIPPET) => "sn",
                        _ => "  ",
                    };
                    let detail = item.detail.as_deref().unwrap_or("");
                    let label = if detail.is_empty() {
                        format!(" {}  {}", kind_str, item.label)
                    } else {
                        format!(" {}  {}  {}", kind_str, item.label, detail)
                    };
                    ListItem::new(Line::from(Span::styled(label, style)))
                }).collect();
                let popup_list = List::new(items)
                    .block(Block::default()
                        .title(" Completion ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(ctx.ui.theme.keyword)))
                    .style(Style::default().bg(ctx.ui.theme.cursor_line));
                frame.render_widget(popup_list, popup_rect);
            }

            if ctx.ui.layout.show_markdown_preview {
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
                ctx.editor.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.editor.buffers.get_mut(ctx.editor.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
            }
            KeyCode::Enter => {
                let cmd = ctx.editor.editor_mode.command_buffer.clone();
                ctx.editor.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.editor.buffers.get_mut(ctx.editor.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
                if !cmd.is_empty() {
                    let action = Action::ExecuteCommand(cmd);
                    let _ = ctx.handle_action(&action);
                }
            }
            KeyCode::Backspace => {
                ctx.editor.editor_mode.command_buffer.pop();
            }
            KeyCode::Char(c) => {
                ctx.editor.editor_mode.command_buffer.push(c);
            }
            _ => {}
        }
    }

    fn handle_search_mode(ctx: &mut AppContext, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                ctx.editor.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.editor.buffers.get_mut(ctx.editor.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
                ctx.editor.search_state = crate::core::SearchState::default();
            }
            KeyCode::Enter => {
                let query = ctx.editor.editor_mode.search_buffer.clone();
                ctx.editor.editor_mode.switch_to_normal();
                if let Some(buf) = ctx.editor.buffers.get_mut(ctx.editor.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
                if !query.is_empty() {
                    ctx.editor.search_state.query = query;
                    ctx.editor.search_state.direction = crate::core::SearchDirection::Forward;
                    let _ = ctx.handle_action(&Action::FindNext);
                }
            }
            KeyCode::Backspace => {
                ctx.editor.editor_mode.search_buffer.pop();
            }
            KeyCode::Char(c) => {
                ctx.editor.editor_mode.search_buffer.push(c);
            }
            _ => {}
        }
    }

}

fn encode_key(key: KeyEvent) -> String {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Enter => "\r".to_string(),
        KeyCode::Tab => "\t".to_string(),
        KeyCode::Backspace => "\x7f".to_string(),
        KeyCode::Esc => "\x1b".to_string(),
        KeyCode::Left => "\x1b[D".to_string(),
        KeyCode::Right => "\x1b[C".to_string(),
        KeyCode::Up => "\x1b[A".to_string(),
        KeyCode::Down => "\x1b[B".to_string(),
        KeyCode::Home => "\x1b[H".to_string(),
        KeyCode::End => "\x1b[F".to_string(),
        KeyCode::PageUp => "\x1b[5~".to_string(),
        KeyCode::PageDown => "\x1b[6~".to_string(),
        KeyCode::Delete => "\x1b[3~".to_string(),
        KeyCode::Insert => "\x1b[2~".to_string(),
        KeyCode::F(n) => {
            let code = match n {
                1 => "OP", 2 => "OQ", 3 => "OR", 4 => "OS",
                5 => "15~", 6 => "17~", 7 => "18~", 8 => "19~",
                9 => "20~", 10 => "21~", 11 => "23~", 12 => "24~",
                _ => return String::new(),
            };
            format!("\x1b[{}", code)
        }
        KeyCode::Char(c) => {
            let mut s = String::new();
            if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                let code = if (c as u8) >= 32 {
                    (c as u8) & 0x1f
                } else {
                    c as u8
                };
                s.push(char::from(code));
                return s;
            }
            if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
                s.push('\x1b');
            }
            s.push(c);
            s
        }
        _ => String::new(),
    }
}
