use std::path::Path;
use crate::core::{Position, Range, EditMode, Notification, BufferInfo, SearchState, Movement, SearchDirection};
use crate::core::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::editor::modes::EditorMode;
use crate::editor::history::{History, HistoryEntry, ChangeKind};
use crate::editor::operations::EditOperations;
use crate::commands::keymap::KeyMap;
use crate::commands::registry::CommandRegistry;
use crate::commands::actions::Action;
use crate::commands::palette::{CommandPalette, PaletteMode};
use crate::config::Config;
use crate::theme::Theme;
use crate::workspace::file_tree::FileTree;
use crate::workspace::search::WorkspaceSearcher;
use crate::async_tasks::task_queue::TaskQueue;
use crate::rendering::engine::RenderEngine;
use crate::ui::UILayout;
use crate::ui::split::SplitManager;
use crate::terminal::TerminalPanel;
use crate::lsp::LspCommand;
use tokio::sync::mpsc;

pub struct AppContext {
    pub buffers: Vec<Buffer>,
    pub active_buffer: usize,
    pub editor_mode: EditorMode,
    pub cursor: Cursor,
    pub selection: Selection,
    pub histories: Vec<History>,
    pub keymap: KeyMap,
    pub registry: CommandRegistry,
    pub config: Config,
    pub theme: Theme,
    pub notifications: Vec<Notification>,
    pub quit_requested: bool,
    pub force_quit: bool,
    pub palette: CommandPalette,
    pub layout: UILayout,
    pub render_engine: RenderEngine,
    pub file_tree: Option<FileTree>,
    pub searcher: Option<WorkspaceSearcher>,
    pub task_queue: TaskQueue,
    pub clipboard: Option<arboard::Clipboard>,
    pub search_state: SearchState,
    pub last_action: Option<Action>,
    pub is_recording: bool,
    pub recorded_macro: Vec<Action>,
    pub start_time: std::time::Instant,
    pub split_manager: SplitManager,
    pub awaiting_split_key: bool,
    pub cached_files: Vec<String>,
    pub cached_files_root: std::path::PathBuf,
    pub cached_files_stamp: std::time::Instant,
    pub git_manager: crate::git::GitManager,
    pub staging_panel: crate::git::StagingPanel,
    pub branch_view: crate::git::BranchViewPanel,
    pub terminal_panel: TerminalPanel,
    pub show_blame: bool,
    pub git_branch: Option<String>,
    pub lsp_tx: Option<mpsc::UnboundedSender<LspCommand>>,
    pub lsp_diagnostics: Vec<lsp_types::Diagnostic>,
    pub completion_items: Vec<lsp_types::CompletionItem>,
    pub show_completion: bool,
    pub completion_selected: usize,
    pub last_keypress: std::time::Instant,
    pub completion_pending: bool,
    pub lsp_enabled: bool,
}

impl AppContext {
    pub fn new(config: Config) -> Self {
        let mut layout = crate::ui::layout::UILayout::new();
        if config.markdown.preview {
            layout.show_markdown_preview = true;
        }
        let theme = Theme::from_name(&config.theme);

        let mut buffers = Vec::new();
        let mut histories = Vec::new();

        let initial_buffer = Buffer::new(0);
        buffers.push(initial_buffer);
        histories.push(History::new(100));

        let file_tree = config
            .workspace
            .root_path
            .as_ref()
            .map(|root| {
                let mut ft = FileTree::new(root.clone());
                let _ = ft.refresh();
                ft
            });

        let searcher = config
            .workspace
            .root_path
            .as_ref()
            .map(|root| WorkspaceSearcher::new(root.clone()));

        let clipboard = arboard::Clipboard::new().ok();

        let split_buf_id = 0;
        let root = config.workspace.root_path.clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        Self {
            buffers,
            active_buffer: 0,
            editor_mode: EditorMode::new(),
            cursor: Cursor::new(),
            selection: Selection::new(),
            histories,
            keymap: KeyMap::new_with_defaults(),
            registry: CommandRegistry::new(),
            terminal_panel: {
                let mut tp = TerminalPanel::new();
                tp.position = match config.terminal.position.as_str() {
                    "top" => crate::terminal::TerminalPosition::Top,
                    "right" => crate::terminal::TerminalPosition::Right,
                    _ => crate::terminal::TerminalPosition::Bottom,
                };
                tp.height = config.terminal.height;
                tp.width = config.terminal.width;
                tp
            },
            config,
            theme,
            notifications: Vec::new(),
            quit_requested: false,
            force_quit: false,
            palette: CommandPalette::new(),
            layout,
            render_engine: RenderEngine::new(),
            file_tree,
            searcher,
            task_queue: TaskQueue::new(4),
            clipboard,
            search_state: SearchState::default(),
            last_action: None,
            is_recording: false,
            recorded_macro: Vec::new(),
            start_time: std::time::Instant::now(),
            split_manager: SplitManager::new(split_buf_id),
            awaiting_split_key: false,
            cached_files: Vec::new(),
            cached_files_root: root.clone(),
            cached_files_stamp: std::time::Instant::now(),
            git_manager: crate::git::GitManager::new(),
            staging_panel: crate::git::StagingPanel::new(),
            branch_view: crate::git::BranchViewPanel::new(),
            show_blame: false,
            git_branch: None,
            lsp_tx: None,
            lsp_diagnostics: Vec::new(),
            completion_items: Vec::new(),
            show_completion: false,
            completion_selected: 0,
            last_keypress: std::time::Instant::now(),
            completion_pending: false,
            lsp_enabled: true,
        }
    }

    pub fn active_buffer(&self) -> &Buffer {
        &self.buffers[self.active_buffer]
    }

    pub fn active_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active_buffer]
    }

    pub fn push_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
        if self.notifications.len() > 10 {
            self.notifications.remove(0);
        }
    }

    pub fn push_info(&mut self, msg: impl Into<String>) {
        self.push_notification(Notification::info(msg));
    }

    pub fn push_error(&mut self, msg: impl Into<String>) {
        self.push_notification(Notification::error(msg));
    }

    pub fn push_success(&mut self, msg: impl Into<String>) {
        self.push_notification(Notification::success(msg));
    }

    pub fn switch_buffer(&mut self, id: usize) -> bool {
        if id < self.buffers.len() {
            self.sync_to_pane();
            self.active_buffer = id;
            if let Some(pane) = self.split_manager.active_pane() {
                pane.buffer_id = id;
            }
            self.cursor.position = self.buffers[id].cursor;
            self.cursor.preferred_column = self.buffers[id].cursor.column;
            self.editor_mode.mode = self.buffers[id].mode;
            self.selection.clear();
            true
        } else {
            false
        }
    }

    pub fn open_file(&mut self, path: std::path::PathBuf) -> Result<usize, anyhow::Error> {
        let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
        let existing = {
            let mut result = None;
            for (i, buf) in self.buffers.iter().enumerate() {
                if let Some(ref bp) = buf.path {
                    if *bp == canonical || *bp == path {
                        result = Some((i, buf.cursor, buf.mode));
                        break;
                    }
                }
            }
            result
        };
        if let Some((i, cursor_pos, mode)) = existing {
            self.sync_to_pane();
            self.active_buffer = i;
            if let Some(pane) = self.split_manager.active_pane() {
                pane.buffer_id = i;
            }
            self.cursor.position = cursor_pos;
            self.cursor.preferred_column = cursor_pos.column;
            self.editor_mode.mode = mode;
            self.selection.clear();
            return Ok(i);
        }

        let id = self.buffers.len();
        let buffer = Buffer::from_path(id, path.clone())?;
        self.buffers.push(buffer);
        self.histories.push(History::new(100));
        self.sync_to_pane();
        self.active_buffer = id;
        self.send_did_open_to_lsp(id);
        if let Some(pane) = self.split_manager.active_pane() {
            pane.buffer_id = id;
        }
        self.cursor = Cursor::new();
        self.editor_mode.mode = EditMode::Normal;
        self.selection.clear();
        Ok(id)
    }

    pub fn close_current_buffer(&mut self) -> bool {
        if self.buffers.len() <= 1 {
            return false;
        }

        let buf = &self.buffers[self.active_buffer];
        if buf.dirty && !self.force_quit {
            return false;
        }

        if self.lsp_enabled {
            self.send_lsp(LspCommand::DidClose { doc_id: self.active_buffer });
        }

        self.sync_to_pane();
        self.buffers.remove(self.active_buffer);
        self.histories.remove(self.active_buffer);

        if self.active_buffer >= self.buffers.len() {
            self.active_buffer = self.buffers.len().saturating_sub(1);
        }

        if let Some(pane) = self.split_manager.active_pane() {
            pane.buffer_id = self.active_buffer;
        }

        self.cursor.position = self.buffers[self.active_buffer].cursor;
        self.cursor.preferred_column = self.buffers[self.active_buffer].cursor.column;
        self.editor_mode.mode = self.buffers[self.active_buffer].mode;
        self.selection.clear();
        true
    }

    pub fn current_buffer_info(&self) -> BufferInfo {
        let buf = self.active_buffer();
        BufferInfo {
            id: buf.id,
            path: buf.path.clone(),
            name: buf.name.clone(),
            is_dirty: buf.dirty,
            is_modified: buf.dirty,
            line_count: buf.line_count(),
            cursor: self.cursor.position,
            mode: buf.mode,
        }
    }

    pub fn sync_from_pane(&mut self) {
        if let Some(pane) = self.split_manager.active_pane() {
            self.cursor = pane.cursor.clone();
            self.selection = pane.selection.clone();
            self.active_buffer = pane.buffer_id;
            self.render_engine.scroll_offset = pane.scroll_offset;
        }
    }

    pub fn sync_to_pane(&mut self) {
        if let Some(pane) = self.split_manager.active_pane() {
            pane.cursor = self.cursor.clone();
            pane.selection = self.selection.clone();
            pane.buffer_id = self.active_buffer;
            pane.scroll_offset = self.render_engine.scroll_offset;
        }
    }

    fn is_modification_action(&self, action: &Action) -> bool {
        matches!(action,
            Action::InsertChar(_) | Action::InsertNewline | Action::InsertTab
            | Action::DeleteBackward | Action::DeleteForward | Action::DeleteCharForward
            | Action::DeleteCharBackward | Action::DeleteWordForward | Action::DeleteWordBackward
            | Action::DeleteLine | Action::DeleteToEndOfLine
            | Action::Paste | Action::Cut | Action::CutLine
            | Action::Indent | Action::Unindent | Action::IndentLine | Action::UnindentLine
            | Action::DuplicateLine | Action::MoveLineUp | Action::MoveLineDown
            | Action::JoinLines | Action::ToggleComment
            | Action::Undo | Action::Redo
        )
    }

    pub fn update_cursor(&mut self) {
        let (clamped, line_count) = {
            let buf = self.active_buffer();
            let clamped = buf.clamp_position(self.cursor.position);
            (clamped, buf.line_count())
        };
        self.cursor.position = clamped;
        {
            let buf = self.active_buffer_mut();
            buf.cursor = clamped;
        }
        let vh = if let Some(pane) = self.split_manager.active_pane() {
            pane.viewport_height
        } else {
            self.render_engine.viewport_height
        };
        let scrolloff = self.config.editor.scrolloff.min(vh / 2);
        if vh > 0 {
            let max_scroll = line_count.saturating_sub(vh);
            let cur_scroll = self.render_engine.scroll_offset.line;
            if clamped.line < cur_scroll + scrolloff && cur_scroll > 0 {
                self.render_engine.scroll_offset.line = clamped.line.saturating_sub(scrolloff);
            } else if clamped.line >= cur_scroll + vh.saturating_sub(scrolloff) {
                let target = clamped.line + 1 + scrolloff - vh;
                self.render_engine.scroll_offset.line = target.min(max_scroll);
            }
        }
    }

    pub fn update_mode(&mut self, mode: EditMode) {
        self.editor_mode.set(mode);
        if let Some(buf) = self.buffers.get_mut(self.active_buffer) {
            buf.mode = mode;
        }
    }

    pub fn handle_action(&mut self, action: &Action) -> Result<(), String> {
        if self.is_recording {
            self.recorded_macro.push(action.clone());
        }

        let _mode = self.editor_mode.mode;

        match action {
            Action::SplitHorizontal => {
                self.sync_to_pane();
                self.split_manager.split_horizontal(self.active_buffer);
                self.push_info("Split horizontal");
                self.sync_from_pane();
                self.update_cursor();
                return Ok(());
            }
            Action::SplitVertical => {
                self.sync_to_pane();
                self.split_manager.split_vertical(self.active_buffer);
                self.push_info("Split vertical");
                self.sync_from_pane();
                self.update_cursor();
                return Ok(());
            }
            Action::ClosePane => {
                if self.split_manager.panes_count() <= 1 {
                    self.push_info("Cannot close last pane");
                    return Ok(());
                }
                let active_id = self.split_manager.active_pane_id;
                self.sync_to_pane();
                self.split_manager.close_pane(active_id);
                self.sync_from_pane();
                self.update_cursor();
                return Ok(());
            }
            Action::NextSplit | Action::FocusPaneRight | Action::FocusPaneDown => {
                self.sync_to_pane();
                self.split_manager.focus_next();
                self.sync_from_pane();
                self.update_cursor();
                return Ok(());
            }
            Action::PreviousSplit | Action::FocusPaneLeft | Action::FocusPaneUp => {
                self.sync_to_pane();
                self.split_manager.focus_prev();
                self.sync_from_pane();
                self.update_cursor();
                return Ok(());
            }
            _ => {}
        }

        if self.split_manager.panes_count() > 1 {
            self.sync_from_pane();
        }

        match action {
            Action::MoveLeft => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::Left);
            }
            Action::MoveRight => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::Right);
            }
            Action::MoveUp => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::Up);
            }
            Action::MoveDown => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::Down);
            }
            Action::InsertChar(c) => {
                if self.editor_mode.is_insert() {
                    let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                    let cursor = &mut self.cursor;
                    let change = EditOperations::insert_char(buf, cursor, *c).map_err(|_| "Failed to insert char")?;
                    if let Some(history) = self.histories.get_mut(self.active_buffer) {
                        history.push(HistoryEntry {
                            changes: vec![change],
                            timestamp: std::time::Instant::now(),
                            cursor_before: cursor.position,
                            cursor_after: cursor.position,
                        });
                    }
                }
            }
            Action::InsertNewline => {
                if self.editor_mode.is_insert() {
                    let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                    let cursor = &mut self.cursor;
                    let change = EditOperations::insert_newline(buf, cursor).map_err(|_| "Failed to insert newline")?;
                    if let Some(history) = self.histories.get_mut(self.active_buffer) {
                        history.push(HistoryEntry {
                            changes: vec![change],
                            timestamp: std::time::Instant::now(),
                            cursor_before: cursor.position,
                            cursor_after: cursor.position,
                        });
                    }
                }
            }
            Action::DeleteBackward => {
                if self.selection.is_active && !self.selection.is_empty() {
                    let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                    let change = EditOperations::delete_selection(buf, &self.selection).map_err(|_| "Failed to delete selection")?;
                    if let Some(change) = change {
                        if let Some(range) = self.selection.normalized_range() {
                            self.cursor.position = range.start;
                            self.cursor.preferred_column = range.start.column;
                        }
                        if let Some(history) = self.histories.get_mut(self.active_buffer) {
                            history.push(HistoryEntry {
                                changes: vec![change],
                                timestamp: std::time::Instant::now(),
                                cursor_before: self.cursor.position,
                                cursor_after: self.cursor.position,
                            });
                        }
                    }
                    self.selection.clear();
                } else {
                    let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                    let cursor = &mut self.cursor;
                    let change = EditOperations::delete_char_backward(buf, cursor).map_err(|_| "Failed to delete backward")?;
                    if let Some(change) = change {
                        if let Some(history) = self.histories.get_mut(self.active_buffer) {
                            history.push(HistoryEntry {
                                changes: vec![change],
                                timestamp: std::time::Instant::now(),
                                cursor_before: cursor.position,
                                cursor_after: cursor.position,
                            });
                        }
                    }
                }
            }
            Action::DeleteForward => {
                if self.selection.is_active && !self.selection.is_empty() {
                    let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                    let change = EditOperations::delete_selection(buf, &self.selection).map_err(|_| "Failed to delete selection")?;
                    if let Some(change) = change {
                        if let Some(range) = self.selection.normalized_range() {
                            self.cursor.position = range.start;
                            self.cursor.preferred_column = range.start.column;
                        }
                        if let Some(history) = self.histories.get_mut(self.active_buffer) {
                            history.push(HistoryEntry {
                                changes: vec![change],
                                timestamp: std::time::Instant::now(),
                                cursor_before: self.cursor.position,
                                cursor_after: self.cursor.position,
                            });
                        }
                    }
                    self.selection.clear();
                } else {
                    let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                    let cursor = &mut self.cursor;
                    let change = EditOperations::delete_char_forward(buf, cursor).map_err(|_| "Failed to delete forward")?;
                    if let Some(change) = change {
                        if let Some(history) = self.histories.get_mut(self.active_buffer) {
                            history.push(HistoryEntry {
                                changes: vec![change],
                                timestamp: std::time::Instant::now(),
                                cursor_before: cursor.position,
                                cursor_after: cursor.position,
                            });
                        }
                    }
                }
            }
            Action::SaveFile => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                buf.save().map_err(|e| format!("Save failed: {}", e))?;
                self.push_success("File saved");
                if let Some(ref mut ft) = self.file_tree {
                    let _ = ft.refresh();
                }
                if self.lsp_enabled {
                    self.send_lsp(LspCommand::DidSave { doc_id: self.active_buffer });
                }
            }
            Action::SaveFileAs => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                if let Some(ref path) = buf.path {
                    buf.save_as(path.clone()).map_err(|e| format!("Save failed: {}", e))?;
                    self.push_success("File saved");
                } else {
                    // In a real editor, this would prompt for a filename
                    self.push_error("No path set for current buffer");
                }
            }
            Action::CloseFile => {
                self.close_current_buffer();
            }
            Action::SwitchToInsertMode => {
                self.update_mode(EditMode::Insert);
            }
            Action::SwitchToNormalMode => {
                self.editor_mode.switch_to_normal();
                if let Some(buf) = self.buffers.get_mut(self.active_buffer) {
                    buf.mode = EditMode::Normal;
                }
                self.selection.clear();
                self.palette.visible = false;
                self.layout.show_palette = false;
                self.search_state = SearchState::default();
            }
            Action::SwitchToVisualMode => {
                self.selection.start(self.cursor.position);
                self.update_mode(EditMode::Visual);
            }
            Action::SwitchToVisualLineMode => {
                self.selection.start(self.cursor.position);
                self.update_mode(EditMode::VisualLine);
            }
            Action::SwitchToCommandMode => {
                self.editor_mode.switch_to_command();
                if let Some(buf) = self.buffers.get_mut(self.active_buffer) {
                    buf.mode = EditMode::Command;
                }
            }
            Action::SwitchToSearchMode => {
                self.editor_mode.set(EditMode::Search);
                self.editor_mode.search_buffer.clear();
                if let Some(buf) = self.buffers.get_mut(self.active_buffer) {
                    buf.mode = EditMode::Search;
                }
            }
            Action::Undo => {
                if let Some(history) = self.histories.get_mut(self.active_buffer) {
                    if let Some(entry) = history.undo() {
                        let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                        for change in entry.changes.iter().rev() {
                            match change {
                                ChangeKind::Insert { pos, text } => {
                                    let end_pos = Position::new(pos.line, pos.column + text.chars().count());
                                    let range = Range::new(*pos, end_pos);
                                    buf.delete_range(range);
                                }
                                ChangeKind::Delete { pos, text, range: _ } => {
                                    buf.insert_str(*pos, text);
                                }
                                ChangeKind::Replace { range: _, old: _, new: _ } => {
                                    // Simplified undo for replace
                                }
                                ChangeKind::Indent { line } => {
                                    let start = Position::new(*line, 0);
                                    let end = Position::new(*line, 4);
                                    buf.delete_range(Range::new(start, end));
                                }
                                ChangeKind::Unindent { line } => {
                                    let pos = Position::new(*line, 0);
                                    buf.insert_str(pos, "    ");
                                }
                            }
                        }
                        self.cursor.position = entry.cursor_before;
                        self.cursor.preferred_column = entry.cursor_before.column;
                    } else {
                        self.push_info("Nothing to undo");
                    }
                }
            }
            Action::Redo => {
                if let Some(history) = self.histories.get_mut(self.active_buffer) {
                    if let Some(entry) = history.redo() {
                        let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                        for change in &entry.changes {
                            match change {
                                ChangeKind::Insert { pos, text } => {
                                    buf.insert_str(*pos, text);
                                }
                                ChangeKind::Delete { pos: _, text: _, range } => {
                                    buf.delete_range(*range);
                                }
                                ChangeKind::Replace { range, old: _, new } => {
                                    buf.delete_range(*range);
                                    buf.insert_str(range.start, new);
                                }
                                ChangeKind::Indent { line } => {
                                    let pos = Position::new(*line, 0);
                                    buf.insert_str(pos, "    ");
                                }
                                ChangeKind::Unindent { line } => {
                                    let start = Position::new(*line, 0);
                                    let end = Position::new(*line, 4);
                                    buf.delete_range(Range::new(start, end));
                                }
                            }
                        }
                        self.cursor.position = entry.cursor_after;
                        self.cursor.preferred_column = entry.cursor_after.column;
                    } else {
                        self.push_info("Nothing to redo");
                    }
                }
            }
            Action::Indent => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::indent_line(buf, cursor);
            }
            Action::Unindent => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::unindent_line(buf, cursor);
            }
            Action::DuplicateLine => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::duplicate_line(buf, cursor);
            }
            Action::MoveLineUp => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::move_line_up(buf, cursor);
            }
            Action::MoveLineDown => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::move_line_down(buf, cursor);
            }
            Action::JoinLines => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::join_lines(buf, cursor);
            }
            Action::ToggleComment => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::toggle_comment(buf, cursor);
            }
            Action::WordForward => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::WordForward);
            }
            Action::WordBackward => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::WordBackward);
            }
            Action::StartOfLine => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::StartOfLine);
            }
            Action::EndOfLine => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::EndOfLine);
            }
            Action::StartOfFile => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::StartOfFile);
            }
            Action::EndOfFile => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::EndOfFile);
            }
            Action::PageUp => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::PageUp);
            }
            Action::PageDown => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::PageDown);
            }
            Action::HalfPageUp => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::HalfPageUp);
            }
            Action::HalfPageDown => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::HalfPageDown);
            }
            Action::SelectAll => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                if buf.line_count() > 0 {
                    let start = Position::zero();
                    let end = Position::new(buf.line_count().saturating_sub(1), buf.chars_at_line(buf.line_count().saturating_sub(1)));
                    self.selection.select_all(start, end);
                }
            }
            Action::Cut => {
                if self.selection.is_active && !self.selection.is_empty() {
                    if let Some(range) = self.selection.normalized_range() {
                        let text = {
                            let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                            buf.get_text_in_range(range)
                        };
                        let _ = self.set_clipboard_text(&text);
                        let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                        let change = EditOperations::delete_selection(buf, &self.selection).map_err(|_| "Failed to cut")?;
                        if let Some(change) = change {
                            if let Some(history) = self.histories.get_mut(self.active_buffer) {
                                history.push(HistoryEntry {
                                    changes: vec![change],
                                    timestamp: std::time::Instant::now(),
                                    cursor_before: self.cursor.position,
                                    cursor_after: range.start,
                                });
                            }
                        }
                        self.cursor.position = range.start;
                        self.cursor.preferred_column = range.start.column;
                        self.selection.clear();
                    }
                }
            }
            Action::Copy => {
                if self.selection.is_active && !self.selection.is_empty() {
                    if let Some(range) = self.selection.normalized_range() {
                        let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                        let text = buf.get_text_in_range(range);
                        let _ = self.set_clipboard_text(&text);
                    }
                }
            }
            Action::Paste => {
                if let Some(text) = self.get_clipboard_text() {
                    let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                    let cursor = &mut self.cursor;
                    buf.insert_str(cursor.position, &text);
                    cursor.position = Position::new(cursor.position.line, cursor.position.column + text.len());
                    cursor.preferred_column = cursor.position.column;
                    buf.set_modified();
                }
            }
            Action::CopyLine => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                let line = buf.get_line(self.cursor.position.line);
                let _ = self.set_clipboard_text(line.trim_end_matches('\n').trim_end_matches('\r'));
            }
            Action::DeleteLine => {
                let (line_count, start_idx, end_idx) = {
                    let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                    let line = self.cursor.position.line;
                    let rope = &buf.rope;
                    let line_start = rope.line_to_char(line);
                    let line_end = if line + 1 < rope.len_lines() {
                        rope.line_to_char(line + 1)
                    } else {
                        rope.len_chars()
                    };
                    (rope.len_lines(), line_start, line_end)
                };
                if start_idx >= end_idx { return Ok(()); }
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                buf.rope.remove(start_idx..end_idx);
                if line_count > 1 && self.cursor.position.line >= line_count.saturating_sub(1) {
                    self.cursor.position.line = self.cursor.position.line.saturating_sub(1);
                }
                self.cursor.position.column = 0;
                self.cursor.preferred_column = 0;
                buf.set_modified();
            }
            Action::DeleteToEndOfLine => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                let line = self.cursor.position.line;
                let line_len = buf.chars_at_line(line);
                let range = Range::new(self.cursor.position, Position::new(line, line_len));
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                buf.delete_range(range);
                buf.set_modified();
            }
            Action::InsertTab => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                let cursor = &mut self.cursor;
                let _ = EditOperations::insert_char(buf, cursor, '\t');
            }
            Action::MoveToMatchingBrace => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::MatchingBrace);
            }
            Action::FuzzyFindFile => {
                let root = self.config.workspace.root_path.clone()
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                if self.cached_files.is_empty() || self.cached_files_root != root {
                    self.cached_files.clear();
                    self.push_info("Scanning workspace...");
                    use ignore::WalkBuilder;
                    let now = std::time::Instant::now();
                    let paths: Vec<String> = WalkBuilder::new(&root)
                        .hidden(false)
                        .git_ignore(true)
                        .build()
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
                        .map(|e| e.path().to_string_lossy().to_string())
                        .collect();
                    self.cached_files = paths;
                    self.cached_files_root = root.clone();
                    self.cached_files_stamp = now;
                }
                if self.cached_files.is_empty() {
                    self.push_error("No files found in workspace");
                    return Ok(());
                }
                // Sort: root-level files first, then subdirectory files
                let mut files = self.cached_files.clone();
                files.sort_by(|a, b| {
                    let a_root = Path::new(a).parent() == Some(&root);
                    let b_root = Path::new(b).parent() == Some(&root);
                    if a_root != b_root { b_root.cmp(&a_root) } else { a.cmp(b) }
                });
                self.palette.set_files(files);
                self.palette.show(PaletteMode::Files);
                self.layout.show_palette = true;
            }
            Action::Quit => {
                let has_dirty = self.buffers.iter().any(|b| b.dirty);
                if has_dirty && !self.force_quit {
                    self.push_error("Unsaved changes. Use :q! or force quit to exit");
                } else {
                    self.quit_requested = true;
                }
            }
            Action::ForceQuit => {
                self.force_quit = true;
                self.quit_requested = true;
            }
            Action::Find => {
                self.update_mode(EditMode::Search);
                self.editor_mode.search_buffer.clear();
                self.search_state = SearchState::default();
            }
            Action::FindNext => {
                self.search_state.direction = SearchDirection::Forward;
                self.perform_search();
            }
            Action::FindPrevious => {
                self.search_state.direction = SearchDirection::Backward;
                self.perform_search();
            }
            Action::Replace => {
                self.push_info("Replace not yet implemented");
            }
            Action::ReplaceAll => {
                self.push_info("Replace all not yet implemented");
            }
            Action::GoToLine(Some(line)) => {
                let buf_len = self.buffers[self.active_buffer].line_count();
                let target = line.saturating_sub(1).min(buf_len.saturating_sub(1));
                self.cursor.position = crate::core::Position::new(target, 0);
                self.cursor.preferred_column = 0;
            }
            Action::GoToLine(None) => {
                self.push_info("Go to line number: use :<number>");
            }
            Action::OpenFile => {
                if let Some(ref mut ft) = self.file_tree {
                    if let Some(path) = ft.selected_path() {
                        let _ = self.open_file(path);
                    }
                }
            }
            Action::ShowPalette => {
                self.palette.toggle();
                self.layout.show_palette = self.palette.visible;
            }
            Action::ToggleFileTree => {
                self.layout.show_file_tree = !self.layout.show_file_tree;
                if self.layout.show_file_tree {
                    if self.file_tree.is_none() {
                        if let Some(ref ws) = self.config.workspace.root_path {
                            let mut ft = FileTree::new(ws.clone());
                            let _ = ft.refresh();
                            self.file_tree = Some(ft);
                        } else {
                            let cwd = std::env::current_dir().unwrap_or_default();
                            let mut ft = FileTree::new(cwd);
                            let _ = ft.refresh();
                            self.file_tree = Some(ft);
                        }
                    }
                    let root = self.file_tree.as_ref().map(|ft| ft.root.display().to_string()).unwrap_or_default();
                    self.push_info(format!("File tree - arrows navigate, Enter open, Esc back [{}]", root));
                    self.layout.focused_pane = crate::ui::layout::FocusedPane::FileTree;
                } else {
                    self.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                }
            }
            Action::ToggleMarkdownPreview => {
                self.layout.show_markdown_preview = !self.layout.show_markdown_preview;
            }
            Action::FocusPreview => {
            }
            Action::ToggleLineNumbers => {
                self.config.line_numbers.show = !self.config.line_numbers.show;
            }
            Action::ToggleWordWrap | Action::ToggleWrap => {
                self.config.editor.word_wrap = !self.config.editor.word_wrap;
            }
            Action::ToggleSyntaxHighlighting => {
                self.config.editor.syntax_highlighting = !self.config.editor.syntax_highlighting;
            }
            Action::IncreaseScrolloff => {
                self.config.editor.scrolloff = self.config.editor.scrolloff.saturating_add(1).min(20);
            }
            Action::DecreaseScrolloff => {
                self.config.editor.scrolloff = self.config.editor.scrolloff.saturating_sub(1);
            }
            Action::FocusFileTree => {
                self.layout.focused_pane = crate::ui::layout::FocusedPane::FileTree;
                if !self.layout.show_file_tree {
                    let _ = self.handle_action(&Action::ToggleFileTree);
                }
            }
            Action::FocusEditor => {
                self.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
            }
            Action::FocusPaneLeft => {}
            Action::FocusPaneRight => {}
            Action::FocusPaneUp => {}
            Action::FocusPaneDown => {}
            Action::SplitHorizontal => {}
            Action::SplitVertical => {}
            Action::ClosePane => {}
            Action::GitBlameToggle => {
                self.show_blame = !self.show_blame;
                if self.show_blame {
                    if let Some(buf) = self.buffers.get(self.active_buffer) {
                        if let Some(ref path) = buf.path {
                            self.git_manager.get_blame(path);
                        }
                    }
                }
            }
            Action::GitStatus => {
                self.staging_panel.toggle();
                if self.staging_panel.visible {
                    self.layout.show_staging_panel = true;
                    if let Some(buf) = self.buffers.get(self.active_buffer) {
                        if let Some(ref path) = buf.path {
                            self.staging_panel.refresh(&mut self.git_manager, path);
                        }
                    }
                } else {
                    self.layout.show_staging_panel = false;
                }
            }
            Action::GitBranchView => {
                self.branch_view.toggle();
                if self.branch_view.visible {
                    if let Some(buf) = self.buffers.get(self.active_buffer) {
                        if let Some(ref path) = buf.path {
                            if let Some(repo_path) = self.git_manager.discover_repo(path) {
                                self.branch_view.refresh(repo_path);
                            } else {
                                self.push_error("No git repository found");
                                self.branch_view.visible = false;
                                return Ok(());
                            }
                        } else {
                            self.push_error("No file path for current buffer");
                            self.branch_view.visible = false;
                            return Ok(());
                        }
                    }
                }
                self.layout.show_branch_view = self.branch_view.visible;
                self.layout.show_staging_panel = false;
            }
            Action::GitStageFile => {
                if let Some(buf) = self.buffers.get(self.active_buffer) {
                    if let Some(ref path) = buf.path {
                        if let Some(repo_path) = self.git_manager.discover_repo(path) {
                            let relative = path.strip_prefix(&repo_path).unwrap_or(path);
                            if let Err(e) = self.git_manager.stage_file(path, &relative.to_string_lossy()) {
                                self.push_error(format!("Stage failed: {}", e));
                            } else {
                                self.push_success("File staged");
                            }
                        }
                    }
                }
            }
            Action::GitUnstageFile => {
                if let Some(buf) = self.buffers.get(self.active_buffer) {
                    if let Some(ref path) = buf.path {
                        if let Some(repo_path) = self.git_manager.discover_repo(path) {
                            let relative = path.strip_prefix(&repo_path).unwrap_or(path);
                            if let Err(e) = self.git_manager.unstage_file(path, &relative.to_string_lossy()) {
                                self.push_error(format!("Unstage failed: {}", e));
                            } else {
                                self.push_success("File unstaged");
                            }
                        }
                    }
                }
            }
            Action::GitStageHunk => {}
            Action::GitUnstageHunk => {}
            Action::GitStageAll => {
                if let Some(buf) = self.buffers.get(self.active_buffer) {
                    if let Some(ref path) = buf.path.clone() {
                        if let Some(_repo_path) = self.git_manager.discover_repo(&path) {
                            let status = self.git_manager.get_status(&path).cloned();
                            if let Some(s) = status {
                                for entry in &s.entries {
                                    let _ = self.git_manager.stage_file(&path, &entry.path);
                                }
                                self.push_success("All changes staged");
                            }
                        }
                    }
                }
            }
            Action::GitCommit => {
                self.push_info(":commit not yet implemented");
            }
            Action::GitDiff => {}
            Action::ToggleTerminal => {
                self.terminal_panel.toggle();
                self.layout.show_terminal = self.terminal_panel.visible;
                self.layout.terminal_position = self.terminal_panel.position.clone();
                self.layout.terminal_height = self.terminal_panel.height;
                self.layout.terminal_width = self.terminal_panel.width;
                if self.terminal_panel.visible {
                    self.layout.focused_pane = crate::ui::layout::FocusedPane::Terminal;
                    self.terminal_panel.focus();
                } else {
                    self.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                }
                self.layout.show_staging_panel = false;
                self.layout.show_branch_view = false;
            }
            Action::FocusTerminal => {
                if !self.terminal_panel.visible {
                    self.terminal_panel.toggle();
                    self.layout.show_terminal = true;
                }
                self.layout.focused_pane = crate::ui::layout::FocusedPane::Terminal;
                self.terminal_panel.focus();
            }
            Action::TerminalNextTab => {
                self.terminal_panel.next_instance();
            }
            Action::TerminalPrevTab => {
                self.terminal_panel.prev_instance();
            }
            Action::TerminalNewTab => {
                if !self.terminal_panel.visible {
                    self.terminal_panel.toggle();
                    self.layout.show_terminal = true;
                }
                self.terminal_panel.spawn("cmd.exe", "cmd");
                self.layout.focused_pane = crate::ui::layout::FocusedPane::Terminal;
                self.terminal_panel.focus();
            }
            Action::TerminalCloseTab => {
                self.terminal_panel.close_active();
                self.layout.show_terminal = self.terminal_panel.visible;
                if !self.terminal_panel.visible {
                    self.layout.focused_pane = crate::ui::layout::FocusedPane::Editor;
                }
            }
            Action::TerminalScrollUp => {
                self.terminal_panel.scroll_up();
            }
            Action::TerminalScrollDown => {
                self.terminal_panel.scroll_down();
            }
            Action::TerminalCyclePosition => {
                self.terminal_panel.cycle_position();
                self.layout.terminal_position = self.terminal_panel.position.clone();
                if !self.terminal_panel.visible {
                    self.terminal_panel.toggle();
                    self.layout.show_terminal = true;
                }
                self.layout.focused_pane = crate::ui::layout::FocusedPane::Terminal;
                self.terminal_panel.focus();
            }
            Action::ExecuteCommand(cmd) => {
                let lower = cmd.to_lowercase();
                match lower.as_str() {
                    "w" | "write" | "s" | "save" => {
                        let _ = self.handle_action(&Action::SaveFile);
                    }
                    s if s.starts_with("w ") || s.starts_with("write ") || s.starts_with("s ") || s.starts_with("save ") => {
                        let path_str = s.splitn(2, ' ').nth(1).unwrap_or("").trim();
                        if !path_str.is_empty() {
                            let path = std::path::PathBuf::from(path_str);
                            let path = if path.is_relative() {
                                std::env::current_dir().unwrap_or_default().join(&path)
                            } else {
                                path
                            };
                            if let Some(buf) = self.buffers.get_mut(self.active_buffer) {
                                match buf.save_as(path) {
                                    Ok(()) => {
                                        self.push_success("File saved");
                                        if let Some(ref mut ft) = self.file_tree {
                                            let _ = ft.refresh();
                                        }
                                    }
                                    Err(e) => self.push_error(format!("Save failed: {}", e)),
                                }
                            }
                        }
                    }
                    "q" | "quit" => {
                        if self.split_manager.panes_count() > 1 {
                            let _ = self.handle_action(&Action::ClosePane);
                        } else {
                            let has_dirty = self.buffers.iter().any(|b| b.dirty);
                            if has_dirty && !self.force_quit {
                                self.push_error("Unsaved changes. Use 'q!' or force quit");
                            } else {
                                self.quit_requested = true;
                            }
                        }
                    }
                    "q!" => {
                        self.force_quit = true;
                        self.quit_requested = true;
                    }
                    "close" => {
                        let _ = self.handle_action(&Action::ClosePane);
                    }
                    "wq" => {
                        let _ = self.handle_action(&Action::SaveFile);
                        self.quit_requested = true;
                    }
                    s if s.starts_with("e ") || s.starts_with("open ") => {
                        let path_str = s.splitn(2, ' ').nth(1).unwrap_or("").trim();
                        if !path_str.is_empty() {
                            let path = std::path::PathBuf::from(path_str);
                            let _ = self.open_file(path);
                        }
                    }
                    s if s.starts_with("split ") || s.starts_with("sp ") => {
                        let path_str = s.splitn(2, ' ').nth(1).unwrap_or("").trim();
                        if path_str.is_empty() {
                            self.handle_action(&Action::SplitHorizontal).ok();
                        } else {
                            let path = std::path::PathBuf::from(path_str);
                            if let Ok(buf_id) = self.open_file(path) {
                                self.split_manager.split_horizontal(buf_id);
                                self.sync_from_pane();
                            }
                        }
                    }
                    "split" | "sp" => {
                        self.handle_action(&Action::SplitHorizontal).ok();
                    }
                    s if s.starts_with("vsplit ") || s.starts_with("vs ") => {
                        let path_str = s.splitn(2, ' ').nth(1).unwrap_or("").trim();
                        if path_str.is_empty() {
                            self.handle_action(&Action::SplitVertical).ok();
                        } else {
                            let path = std::path::PathBuf::from(path_str);
                            if let Ok(buf_id) = self.open_file(path) {
                                self.split_manager.split_vertical(buf_id);
                                self.sync_from_pane();
                            }
                        }
                    }
                    "vsplit" | "vs" => {
                        self.handle_action(&Action::SplitVertical).ok();
                    }
                    "blame" => {
                        self.handle_action(&Action::GitBlameToggle).ok();
                    }
                    "status" | "st" => {
                        self.handle_action(&Action::GitStatus).ok();
                    }
                    "branch" | "branches" | "br" => {
                        self.handle_action(&Action::GitBranchView).ok();
                    }
                    "terminal" | "term" | "t" => {
                        self.handle_action(&Action::ToggleTerminal).ok();
                    }
                    s if s.starts_with("terminal ") || s.starts_with("term ") => {
                        let shell = s.splitn(2, ' ').nth(1).unwrap_or("").trim();
                        if !shell.is_empty() {
                            self.terminal_panel.spawn(shell, shell);
                            self.layout.show_terminal = true;
                            self.layout.focused_pane = crate::ui::layout::FocusedPane::Terminal;
                            self.terminal_panel.focus();
                        } else {
                            self.handle_action(&Action::ToggleTerminal).ok();
                        }
                    }
                    "help" => {
                        self.push_info("tflow: :w save, :w <file> save as, :q quit, :e <file> open, :sp/:vs split, :new/:vnew buffer, :blame, :status, :branch, :terminal, Ctrl+P find file, :help help");
                    }
                    "new" => {
                        let id = self.buffers.len();
                        self.buffers.push(Buffer::new(id));
                        self.histories.push(History::new(100));
                        self.sync_to_pane();
                        self.split_manager.split_horizontal(id);
                        self.active_buffer = id;
                        self.cursor = crate::editor::cursor::Cursor::new();
                        self.editor_mode.mode = EditMode::Normal;
                        self.selection.clear();
                        self.sync_from_pane();
                    }
                    "vnew" => {
                        let id = self.buffers.len();
                        self.buffers.push(Buffer::new(id));
                        self.histories.push(History::new(100));
                        self.sync_to_pane();
                        self.split_manager.split_vertical(id);
                        self.active_buffer = id;
                        self.cursor = crate::editor::cursor::Cursor::new();
                        self.editor_mode.mode = EditMode::Normal;
                        self.selection.clear();
                        self.sync_from_pane();
                    }
                    _ => {
                        self.push_error(format!("Unknown command: {}", cmd));
                    }
                }
            }
            Action::RepeatLastAction => {
                if let Some(ref last) = self.last_action.clone() {
                    let _ = self.handle_action(last);
                }
            }
            Action::ToggleRecording => {
                self.is_recording = !self.is_recording;
                if self.is_recording {
                    self.recorded_macro.clear();
                    self.push_info("Recording macro");
                } else {
                    self.push_info("Recording stopped");
                }
            }
            Action::PlaybackMacro => {
                let macro_actions = self.recorded_macro.clone();
                for action in &macro_actions {
                    let _ = self.handle_action(action);
                }
            }
            Action::Noop => {}
            Action::ReloadFile => {
                let buf = self.buffers.get_mut(self.active_buffer).ok_or("No active buffer")?;
                buf.load().map_err(|e| format!("Reload failed: {}", e))?;
                self.push_success("File reloaded");
            }
            Action::NewFile => {
                let id = self.buffers.len();
                let buffer = Buffer::new(id);
                self.buffers.push(buffer);
                self.histories.push(History::new(100));
                self.active_buffer = id;
                self.cursor = Cursor::new();
                self.editor_mode.mode = EditMode::Normal;
                self.selection.clear();
            }
            Action::ScrollUp => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::Up);
            }
            Action::ScrollDown => {
                let buf = self.buffers.get(self.active_buffer).ok_or("No active buffer")?;
                EditOperations::apply_movement(buf, &mut self.cursor, Movement::Down);
            }
            Action::OpenFileAt(path) => {
                let path = std::path::PathBuf::from(path);
                let _ = self.open_file(path);
            }
            Action::SwitchBuffer(id) => {
                self.switch_buffer(*id);
            }
            Action::NextBuffer => {
                let next = (self.active_buffer + 1) % self.buffers.len();
                self.switch_buffer(next);
            }
            Action::PreviousBuffer => {
                let prev = if self.active_buffer == 0 {
                    self.buffers.len().saturating_sub(1)
                } else {
                    self.active_buffer - 1
                };
                self.switch_buffer(prev);
            }
            Action::MoveToLine(line) => {
                let buf_len = self.buffers[self.active_buffer].line_count();
                let target = line.saturating_sub(1).min(buf_len.saturating_sub(1));
                self.cursor.position = Position::new(target, 0);
                self.cursor.preferred_column = 0;
            }
            Action::JumpToLine => {
                self.push_info("Jump to line: use :<number> or :goto <line>");
            }
            Action::Suspend => {}
            Action::DebugInfo => {}
            Action::ReloadConfig => {}
            _ => {}
        }

        if *action != Action::Noop && *action != Action::NoOp {
            self.last_action = Some(action.clone());
        }

        if !self.search_state.query.is_empty() && self.is_modification_action(action) {
            self.search_state = SearchState::default();
        }

        if self.is_modification_action(action) && self.lsp_enabled {
            self.notify_lsp_change(action);
        }

        self.update_cursor();
        self.sync_to_pane();
        Ok(())
    }

    fn perform_search(&mut self) {
        let query = self.editor_mode.search_buffer.clone();
        if query.is_empty() {
            return;
        }
        self.search_state.query = query.clone();
        let buf = self.buffers.get(self.active_buffer).ok_or(()).unwrap();
        let text = buf.get_text();
        let mut matches = Vec::new();
        let lower_query = query.to_lowercase();
        for (line_idx, line) in text.lines().enumerate() {
            let search_line = if self.search_state.case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            let search_q = if self.search_state.case_sensitive {
                query.clone()
            } else {
                lower_query.clone()
            };
            let mut start_col = 0;
            while let Some(col) = if self.search_state.is_regex {
                if let Ok(re) = regex::Regex::new(&search_q) {
                    re.find(&search_line[start_col..]).map(|m| start_col + m.start())
                } else {
                    search_line[start_col..].find(&search_q).map(|c| start_col + c)
                }
            } else {
                search_line[start_col..].find(&search_q).map(|c| start_col + c)
            } {
                matches.push(Position::new(line_idx, col));
                start_col = col + 1;
                if start_col >= search_line.len() {
                    break;
                }
            }
        }
        self.search_state.matches = matches;

        if !self.search_state.matches.is_empty() {
            let current = self.cursor.position;
            let total = self.search_state.matches.len();
            let start = self.search_state.current_match;
            let mut best = None;
            match self.search_state.direction {
                SearchDirection::Forward => {
                    if let Some(s) = start {
                        best = Some((s + 1) % total);
                    } else {
                        for (i, m) in self.search_state.matches.iter().enumerate() {
                            if *m >= current {
                                best = Some(i);
                                break;
                            }
                        }
                        if best.is_none() {
                            best = Some(0);
                        }
                    }
                }
                SearchDirection::Backward => {
                    if let Some(s) = start {
                        best = Some((s + total - 1) % total);
                    } else {
                        for (i, m) in self.search_state.matches.iter().enumerate().rev() {
                            if *m <= current {
                                best = Some(i);
                                break;
                            }
                        }
                        if best.is_none() {
                            best = Some(total - 1);
                        }
                    }
                }
            }
            self.search_state.current_match = best;
            if let Some(idx) = best {
                if let Some(pos) = self.search_state.matches.get(idx) {
                    self.cursor.position = *pos;
                    self.cursor.preferred_column = pos.column;
                }
            }
        }
    }

    pub fn tick(&mut self) {
        self.notifications.retain(|n| !n.expired());
        self.cursor.toggle_blink();
    }

    pub fn get_clipboard_text(&mut self) -> Option<String> {
        self.clipboard.as_mut().and_then(|c| c.get_text().ok())
    }

    pub fn set_clipboard_text(&mut self, text: &str) -> Result<(), String> {
        self.clipboard
            .as_mut()
            .ok_or_else(|| "No clipboard available".to_string())?
            .set_text(text.to_string())
            .map_err(|e| format!("Clipboard error: {}", e))
    }

    pub fn send_lsp(&self, cmd: LspCommand) {
        if let Some(ref tx) = self.lsp_tx {
            let _ = tx.send(cmd);
        }
    }

    pub fn notify_lsp_change(&self, action: &Action) {
        let buf = match self.buffers.get(self.active_buffer) {
            Some(b) => b,
            None => return,
        };
        let path = match &buf.path {
            Some(p) => p.clone(),
            None => return,
        };
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => return,
        };
        let lang_config = crate::lsp::config::LanguageServerConfig::new();
        let lang_id = match lang_config.language_for_extension(ext) {
            Some(l) => l.clone(),
            None => return,
        };
        if !lang_config.has_server_for(&lang_id) {
            return;
        }

        let change = self.build_change_from_action(action, buf);
        let _ = self.lsp_tx.as_ref().map(|tx| {
            tx.send(LspCommand::DidChange {
                doc_id: self.active_buffer,
                version: 0,
                changes: vec![change],
            })
        });
    }

    fn build_change_from_action(&self, _action: &Action, buf: &Buffer) -> lsp_types::TextDocumentContentChangeEvent {
        // Always send full text to avoid range-mismatch bugs with incremental sync.
        let full_text = buf.get_text();
        lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: full_text,
        }
    }

    pub fn send_did_open_to_lsp(&self, buffer_id: usize) {
        let buf = match self.buffers.get(buffer_id) {
            Some(b) => b,
            None => return,
        };
        let path = match &buf.path {
            Some(p) => p.clone(),
            None => return,
        };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang_id = crate::lsp::config::LanguageServerConfig::new()
            .language_for_extension(ext)
            .cloned()
            .unwrap_or_else(|| ext.to_string());
        let text = buf.get_text();
        self.send_lsp(LspCommand::DidOpen {
            doc_id: buffer_id,
            path,
            language_id: lang_id,
            text,
            version: 1,
        });
    }

    pub fn trigger_completion(&mut self) {
        self.completion_pending = false;
        let buf = match self.buffers.get(self.active_buffer) {
            Some(b) => b,
            None => return,
        };

        // Only trigger when there's a word prefix at the cursor position
        let line_text = buf.get_line(self.cursor.position.line);
        let col = self.cursor.position.column.min(line_text.len());
        let has_prefix = line_text[..col]
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .next()
            .is_some();
        if !has_prefix {
            return;
        }

        let pos = lsp_types::Position::new(
            self.cursor.position.line as u32,
            crate::lsp::types::char_offset_to_utf16(&line_text, self.cursor.position.column),
        );
        let ext = buf.path.as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let lang_config = crate::lsp::config::LanguageServerConfig::new();
        if !lang_config.has_server_for(lang_config.language_for_extension(ext).unwrap_or(&ext.to_string())) {
            return;
        }
        if self.lsp_tx.is_none() {
            self.push_info("LSP: no channel (server not initialized)");
            return;
        }
        self.push_info("LSP completion request sent");
        self.send_lsp(LspCommand::Completion {
            doc_id: self.active_buffer,
            position: pos,
            trigger_kind: Some(lsp_types::CompletionTriggerKind::INVOKED),
            trigger_character: None,
        });
    }

    pub fn accept_completion(&mut self) {
        let items = std::mem::take(&mut self.completion_items);
        self.show_completion = false;
        let idx = self.completion_selected;
        if idx >= items.len() { return; }
        let item = &items[idx];
        let text = item.insert_text.as_deref().unwrap_or(&item.label).to_string();
        // Delete the word being completed (from cursor back to last non-word char)
        let buf = match self.buffers.get(self.active_buffer) {
            Some(b) => b,
            None => return,
        };
        let line_text = buf.get_line(self.cursor.position.line);
        let col = self.cursor.position.column;
        let prefix_len = line_text[..col.min(line_text.len())]
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .count();
        for _ in 0..prefix_len {
            let _ = self.handle_action(&Action::DeleteBackward);
        }
        // Insert the completion text
        for ch in text.chars() {
            let action = if ch == '\n' {
                Action::InsertNewline
            } else {
                Action::InsertChar(ch)
            };
            let _ = self.handle_action(&action);
        }
    }

    pub fn update_title(&self) -> String {
        let buf = self.active_buffer();
        let modified = if buf.dirty { " [+] " } else { "" };
        let name = &buf.name;
        let mode = match self.editor_mode.mode {
            EditMode::Normal => "NORMAL",
            EditMode::Insert => "INSERT",
            EditMode::Visual => "VISUAL",
            EditMode::VisualLine => "VISUAL LINE",
            EditMode::Command => "COMMAND",
            EditMode::Search => "SEARCH",
        };
        let pane_count = self.split_manager.panes_count();
        let pane_info = if pane_count > 1 {
            format!(" [{}:{}]", self.split_manager.active_pane_id, pane_count)
        } else {
            String::new()
        };
        format!("tflow - {}{} - {}{} - {}:{}", name, modified, mode, pane_info, self.cursor.position.line + 1, self.cursor.position.column + 1)
    }

    pub fn total_elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}




