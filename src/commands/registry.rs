use crate::commands::actions::Action;
use std::collections::HashMap;

pub type CommandFn = Box<dyn Fn(&mut crate::app::AppContext) -> Result<(), String> + Send + Sync>;

#[derive(Debug, Clone)]
pub struct CommandArg {
    pub name: String,
    pub arg_type: ArgType,
    pub required: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgType {
    String,
    Number,
    Bool,
    FilePath,
    FilePattern,
}

pub struct RegisteredCommand {
    pub name: String,
    pub description: String,
    pub action: Option<Action>,
    pub handler: Option<CommandFn>,
    pub args: Vec<CommandArg>,
}

impl std::fmt::Debug for RegisteredCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredCommand")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("action", &self.action)
            .field("handler", &self.handler.is_some())
            .field("args", &self.args)
            .finish()
    }
}

impl Clone for RegisteredCommand {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            action: self.action.clone(),
            handler: None,
            args: self.args.clone(),
        }
    }
}

impl RegisteredCommand {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            action: None,
            handler: None,
            args: Vec::new(),
        }
    }
}

fn find_matching_brace_char(buffer: &crate::core::buffer::Buffer, pos: crate::core::Position) -> Option<crate::core::Position> {
    let pairs = [('(', ')'), ('{', '}'), ('[', ']')];
    let c = buffer.char_at(pos)?;
    for &(open, close) in &pairs {
        if c == open {
            return find_forward_match(buffer, pos, open, close);
        }
        if c == close {
            return find_backward_match(buffer, pos, open, close);
        }
    }
    None
}

fn find_forward_match(buffer: &crate::core::buffer::Buffer, pos: crate::core::Position, open: char, close: char) -> Option<crate::core::Position> {
    let mut depth = 1;
    let total_lines = buffer.line_count();
    let mut line = pos.line;
    let mut col = pos.column + 1;
    loop {
        if line >= total_lines {
            return None;
        }
        let line_len = buffer.chars_at_line(line);
        if col >= line_len {
            line += 1;
            col = 0;
            continue;
        }
        let c = buffer.char_at(crate::core::Position::new(line, col))?;
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(crate::core::Position::new(line, col));
            }
        }
        col += 1;
    }
}

fn find_backward_match(buffer: &crate::core::buffer::Buffer, pos: crate::core::Position, open: char, close: char) -> Option<crate::core::Position> {
    let mut depth = 1;
    let mut line = pos.line;
    let mut col = pos.column;
    if col == 0 {
        if line == 0 {
            return None;
        }
        line -= 1;
        col = buffer.chars_at_line(line);
    } else {
        col -= 1;
    }
    loop {
        let c = buffer.char_at(crate::core::Position::new(line, col))?;
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some(crate::core::Position::new(line, col));
            }
        }
        if col == 0 {
            if line == 0 {
                return None;
            }
            line -= 1;
            col = buffer.chars_at_line(line);
        } else {
            col -= 1;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandRegistry {
    pub commands: HashMap<String, RegisteredCommand>,
    pub aliases: HashMap<String, String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        };
        reg.register_defaults();
        reg
    }

    fn register_defaults(&mut self) {
        self.register("write", "Save the current buffer", Action::SaveFile);
        self.register("w", "Save the current buffer (alias)", Action::SaveFile);
        self.register("quit", "Quit the editor", Action::Quit);
        self.register("q", "Quit the editor (alias)", Action::Quit);
        self.register("wq", "Save and quit", Action::SaveFile);
        self.register("x", "Save and quit", Action::SaveFile);
        self.register_alias("x", "wq");
        self.register_fn("q!", "Force quit without saving", Box::new(|ctx| {
            ctx.app.force_quit = true;
            ctx.app.quit_requested = true;
            Ok(())
        }));
        self.register("edit", "Open a file for editing", Action::OpenFile);
        self.register("e", "Open a file (alias)", Action::OpenFile);
        self.register_alias("e", "edit");
        self.register("new", "Create a new buffer", Action::NewFile);
        self.register("split", "Split window horizontally", Action::SplitHorizontal);
        self.register("vsplit", "Split window vertically", Action::SplitVertical);
        self.register("tabnew", "Open file in new tab", Action::NewFile);
        self.register("bd", "Delete current buffer", Action::CloseFile);
        self.register("bdelete", "Delete current buffer", Action::CloseFile);
        self.register_alias("bdelete", "bd");
        self.register("bn", "Go to next buffer", Action::NextBuffer);
        self.register("bnext", "Go to next buffer", Action::NextBuffer);
        self.register_alias("bnext", "bn");
        self.register("bp", "Go to previous buffer", Action::PreviousBuffer);
        self.register("bprevious", "Go to previous buffer", Action::PreviousBuffer);
        self.register_alias("bprevious", "bp");
        self.register("buffers", "List all buffers", Action::Noop);
        self.register("ls", "List all buffers (alias)", Action::Noop);
        self.register_alias("ls", "buffers");
        self.register("help", "Show help", Action::ShowPalette);
        self.register("set", "Set an option", Action::Noop);
        self.register("tabedit", "Edit file in new tab", Action::OpenFile);
        self.register("tabclose", "Close current tab", Action::CloseFile);
        self.register("only", "Close all other splits", Action::ClosePane);
        self.register("undo", "Undo last change", Action::Undo);
        self.register("redo", "Redo last change", Action::Redo);
        self.register("copy", "Copy selection", Action::Copy);
        self.register("cut", "Cut selection", Action::Cut);
        self.register("paste", "Paste from clipboard", Action::Paste);
        self.register("yank", "Copy selection", Action::Copy);
        self.register("delete", "Delete selection", Action::Cut);
        self.register("selectall", "Select all content", Action::SelectAll);
        self.register("replace", "Search and replace", Action::Replace);
        self.register("find", "Search forward", Action::SearchForward);
        self.register("nohlsearch", "Clear search highlights", Action::Noop);
        self.register("noh", "Clear search highlights (alias)", Action::Noop);
        self.register_alias("noh", "nohlsearch");
        self.register("togglecomment", "Toggle comment on current line", Action::ToggleComment);
        self.register("join", "Join lines", Action::JoinLines);
        self.register("duplicate", "Duplicate line", Action::DuplicateLine);
        self.register("moveup", "Move line up", Action::MoveLineUp);
        self.register("movedown", "Move line down", Action::MoveLineDown);
        self.register("sort", "Sort lines", Action::Noop);
        self.register("preview", "Toggle markdown preview", Action::ToggleMarkdownPreview);
        self.register("previewtoggle", "Toggle markdown preview alias", Action::ToggleMarkdownPreview);
        self.register_alias("previewtoggle", "preview");
        self.register("sidebyside", "Toggle side by side preview", Action::ToggleSideBySide);
        self.register("focuseditor", "Focus editor panel", Action::FocusEditor);
        self.register("focuspreview", "Focus preview panel", Action::FocusPreview);
        self.register("treetoggle", "Toggle file tree", Action::ToggleFileTree);
        self.register("treefocus", "Focus file tree", Action::FocusFileTree);
        self.register("searchworkspace", "Toggle workspace search", Action::ToggleWorkspaceSearch);
        self.register("line", "Go to line number", Action::GoToLine(None));
        self.register("fontsize", "Set font size", Action::ResetFontSize);
        self.register("fontincrease", "Increase font size", Action::IncreaseFontSize);
        self.register("fontdecrease", "Decrease font size", Action::DecreaseFontSize);
        self.register("linenumbers", "Toggle line numbers", Action::ToggleLineNumbers);
        self.register("relativenumbers", "Toggle relative line numbers", Action::ToggleRelativeLineNumbers);
        self.register("wrap", "Toggle line wrap", Action::ToggleWordWrap);
        self.register("reload", "Reload current file", Action::ReloadFile);
        self.register("config", "Reload configuration", Action::ReloadConfig);
        self.register("debug", "Show debug information", Action::DebugInfo);
        self.register("suspend", "Suspend editor", Action::Suspend);
        self.register("stop", "Suspend editor alias", Action::Suspend);
        self.register_alias("stop", "suspend");
    }

    pub fn register(&mut self, name: &str, description: &str, action: Action) {
        let cmd = RegisteredCommand {
            name: name.to_string(),
            description: description.to_string(),
            action: Some(action),
            handler: None,
            args: Vec::new(),
        };
        self.commands.insert(name.to_string(), cmd);
    }

    pub fn register_fn(&mut self, name: &str, description: &str, handler: CommandFn) {
        let cmd = RegisteredCommand {
            name: name.to_string(),
            description: description.to_string(),
            action: None,
            handler: Some(handler),
            args: Vec::new(),
        };
        self.commands.insert(name.to_string(), cmd);
    }

    pub fn register_alias(&mut self, alias: &str, target: &str) {
        self.aliases.insert(alias.to_string(), target.to_string());
    }

    pub fn execute(&self, name: &str, _args: &[String], ctx: &mut crate::app::AppContext) -> Result<(), String> {
        let resolved_name = self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);
        let cmd = self.commands.get(resolved_name).ok_or_else(|| {
            format!("Unknown command: {}", name)
        })?;
        if let Some(ref handler) = cmd.handler {
            return handler(ctx);
        }
        if let Some(ref action) = cmd.action {
            return self.execute_action(action, ctx);
        }
        Err(format!("Command '{}' has no handler or action", name))
    }

    pub fn execute_action(&self, action: &Action, ctx: &mut crate::app::AppContext) -> Result<(), String> {
        match action {
            Action::MoveLeft => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                if buf.cursor.column > 0 {
                    buf.cursor.column -= 1;
                } else if buf.cursor.line > 0 {
                    buf.cursor.line -= 1;
                    buf.cursor.column = buf.chars_at_line(buf.cursor.line);
                }
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::MoveRight => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line_len = buf.chars_at_line(buf.cursor.line);
                if buf.cursor.column < line_len {
                    buf.cursor.column += 1;
                } else if buf.cursor.line + 1 < buf.line_count() {
                    buf.cursor.line += 1;
                    buf.cursor.column = 0;
                }
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::MoveUp => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                if buf.cursor.line > 0 {
                    buf.cursor.line -= 1;
                }
                let line_len = buf.chars_at_line(buf.cursor.line);
                buf.cursor.column = buf.cursor.column.min(line_len);
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::MoveDown => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                if buf.cursor.line + 1 < buf.line_count() {
                    buf.cursor.line += 1;
                }
                let line_len = buf.chars_at_line(buf.cursor.line);
                buf.cursor.column = buf.cursor.column.min(line_len);
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::WordForward => {
                let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let new_pos = crate::editor::EditOperations::word_forward(buf, buf.cursor);
                let _ = buf;
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                buf.cursor = new_pos;
                ctx.editor.cursor.position = new_pos;
            }
            Action::WordBackward => {
                let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let new_pos = crate::editor::EditOperations::word_backward(buf, buf.cursor);
                let _ = buf;
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                buf.cursor = new_pos;
                ctx.editor.cursor.position = new_pos;
            }
            Action::StartOfLine => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                buf.cursor.column = 0;
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::EndOfLine => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line_len = buf.chars_at_line(buf.cursor.line);
                buf.cursor.column = line_len;
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::StartOfFile => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                buf.cursor = crate::core::Position::zero();
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::EndOfFile => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let last_line = buf.line_count().saturating_sub(1);
                let line_len = buf.chars_at_line(last_line);
                buf.cursor = crate::core::Position::new(last_line, line_len);
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::PageUp => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let target = buf.cursor.line.saturating_sub(50);
                let line_len = buf.chars_at_line(target);
                buf.cursor = crate::core::Position::new(target, buf.cursor.column.min(line_len));
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::PageDown => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let last = buf.line_count().saturating_sub(1);
                let target = (buf.cursor.line + 50).min(last);
                let line_len = buf.chars_at_line(target);
                buf.cursor = crate::core::Position::new(target, buf.cursor.column.min(line_len));
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::HalfPageUp => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let target = buf.cursor.line.saturating_sub(25);
                let line_len = buf.chars_at_line(target);
                buf.cursor = crate::core::Position::new(target, buf.cursor.column.min(line_len));
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::HalfPageDown => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let last = buf.line_count().saturating_sub(1);
                let target = (buf.cursor.line + 25).min(last);
                let line_len = buf.chars_at_line(target);
                buf.cursor = crate::core::Position::new(target, buf.cursor.column.min(line_len));
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::MoveToMatchingBrace => {
                let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                if let Some(match_pos) = find_matching_brace_char(buf, pos) {
                let _ = buf;
                    let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                buf.cursor = match_pos;
                    ctx.editor.cursor.position = match_pos;
                }
            }
            Action::MoveToLine(n) => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = (*n).saturating_sub(1).min(buf.line_count().saturating_sub(1));
                let line_len = buf.chars_at_line(line);
                buf.cursor = crate::core::Position::new(line, buf.cursor.column.min(line_len));
                ctx.editor.cursor.position = buf.cursor;
            }
            Action::InsertChar(c) => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                buf.insert_char(pos, *c);
                if *c == '\n' {
                    buf.cursor = crate::core::Position::new(pos.line + 1, 0);
                } else {
                    buf.cursor = crate::core::Position::new(pos.line, pos.column + 1);
                }
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::InsertNewline => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                buf.insert_newline(pos);
                buf.cursor = crate::core::Position::new(pos.line + 1, 0);
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::InsertTab => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                buf.insert_str(pos, "    ");
                buf.cursor = crate::core::Position::new(pos.line, pos.column + 4);
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::DeleteForward => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                let line_len = buf.chars_at_line(pos.line);
                if pos.column < line_len {
                    buf.delete_char(pos);
                    buf.set_modified();
                } else if pos.line + 1 < buf.line_count() {
                    let next_pos = crate::core::Position::new(pos.line + 1, 0);
                    buf.delete_char(next_pos);
                    buf.set_modified();
                }
            }
            Action::DeleteBackward => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                if buf.cursor.column == 0 && buf.cursor.line == 0 {
                    return Ok(());
                }
                if buf.cursor.column > 0 {
                    let prev = crate::core::Position::new(buf.cursor.line, buf.cursor.column - 1);
                    buf.delete_char(prev);
                    buf.cursor = prev;
                } else {
                    let prev_line = buf.cursor.line - 1;
                    let prev_len = buf.chars_at_line(prev_line);
                    let newline_pos = crate::core::Position::new(prev_line, prev_len);
                    buf.delete_char(newline_pos);
                    buf.cursor = crate::core::Position::new(prev_line, prev_len);
                }
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::DeleteWordForward => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                let new_pos = crate::editor::EditOperations::word_forward(buf, pos);
                if new_pos != pos {
                    let range = crate::core::Range::new(pos, new_pos);
                    buf.delete_range(range);
                    buf.set_modified();
                }
            }
            Action::DeleteWordBackward => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                let new_pos = crate::editor::EditOperations::word_backward(buf, pos);
                if new_pos != pos {
                    let range = crate::core::Range::new(new_pos, pos);
                    buf.delete_range(range);
                    buf.cursor = new_pos;
                    ctx.editor.cursor.position = new_pos;
                    buf.set_modified();
                }
            }
            Action::DeleteLine => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                if buf.line_count() > 1 {
                    let start = crate::core::Position::new(line, 0);
                    let end = if line + 1 < buf.line_count() {
                        crate::core::Position::new(line + 1, 0)
                    } else {
                        crate::core::Position::new(line, buf.chars_at_line(line))
                    };
                    buf.delete_range(crate::core::Range::new(start, end));
                    let max_line = buf.line_count().saturating_sub(1);
                    buf.cursor.line = buf.cursor.line.min(max_line);
                    buf.cursor.column = 0;
                    ctx.editor.cursor.position = buf.cursor;
                    buf.set_modified();
                }
            }
            Action::DeleteToEndOfLine => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                let line_len = buf.chars_at_line(line);
                if buf.cursor.column < line_len {
                    let range = crate::core::Range::new(
                        buf.cursor,
                        crate::core::Position::new(line, line_len),
                    );
                    buf.delete_range(range);
                    buf.set_modified();
                }
            }
            Action::JoinLines => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                if line + 1 < buf.line_count() {
                    let line_len = buf.chars_at_line(line);
                    let nl_pos = crate::core::Position::new(line, line_len);
                    buf.delete_char(nl_pos);
                    buf.set_modified();
                }
            }
            Action::Indent => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = crate::core::Position::new(buf.cursor.line, 0);
                buf.insert_str(pos, "    ");
                if buf.cursor.line == pos.line {
                    buf.cursor.column += 4;
                    ctx.editor.cursor.position = buf.cursor;
                }
                buf.set_modified();
            }
            Action::Unindent => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                let line_text = buf.get_line(line);
                let to_remove = line_text.chars().take_while(|c| *c == ' ').take(4).count();
                if to_remove > 0 {
                    let range = crate::core::Range::new(
                        crate::core::Position::new(line, 0),
                        crate::core::Position::new(line, to_remove),
                    );
                    buf.delete_range(range);
                    if buf.cursor.line == line {
                        buf.cursor.column = buf.cursor.column.saturating_sub(to_remove);
                        ctx.editor.cursor.position = buf.cursor;
                    }
                    buf.set_modified();
                }
            }
            Action::DuplicateLine => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                let line_text = buf.get_line(line);
                let insert_pos = crate::core::Position::new(line + 1, 0);
                let text = format!("{}\n", line_text);
                buf.insert_str(insert_pos, &text);
                buf.cursor = crate::core::Position::new(line + 1, buf.cursor.column);
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::MoveLineUp => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                if line == 0 {
                    return Ok(());
                }
                let line_text = buf.get_line(line);
                let above_text = buf.get_line(line - 1);
                let line_start = crate::core::Position::new(line, 0);
                let line_end = if line + 1 < buf.line_count() {
                    crate::core::Position::new(line + 1, 0)
                } else {
                    let l = buf.chars_at_line(line);
                    crate::core::Position::new(line, l)
                };
                buf.delete_range(crate::core::Range::new(line_start, line_end));
                let above_start = crate::core::Position::new(line - 1, 0);
                let above_end = if line < buf.line_count() {
                    crate::core::Position::new(line, 0)
                } else {
                    let l = buf.chars_at_line(line - 1);
                    crate::core::Position::new(line - 1, l)
                };
                buf.delete_range(crate::core::Range::new(above_start, above_end));
                buf.insert_str(crate::core::Position::new(line - 1, 0), &format!("{}\n", line_text));
                let insert_below = if line - 1 < buf.line_count() - 1 {
                    crate::core::Position::new(line, 0)
                } else {
                    let total = buf.line_count();
                    crate::core::Position::new(total, 0)
                };
                buf.insert_str(insert_below, &format!("{}\n", above_text));
                buf.cursor.line = line.saturating_sub(1);
                let new_line_len = buf.chars_at_line(buf.cursor.line);
                buf.cursor.column = buf.cursor.column.min(new_line_len);
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::MoveLineDown => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                if line + 1 >= buf.line_count() {
                    return Ok(());
                }
                let line_text = buf.get_line(line);
                let below_text = buf.get_line(line + 1);
                let line_start = crate::core::Position::new(line, 0);
                let line_end = if line + 1 < buf.line_count() {
                    crate::core::Position::new(line + 1, 0)
                } else {
                    let l = buf.chars_at_line(line);
                    crate::core::Position::new(line, l)
                };
                buf.delete_range(crate::core::Range::new(line_start, line_end));
                let below_start = crate::core::Position::new(line + 1, 0);
                let below_end = if line + 2 < buf.line_count() {
                    crate::core::Position::new(line + 2, 0)
                } else {
                    let l = buf.chars_at_line(line + 1);
                    crate::core::Position::new(line + 1, l)
                };
                buf.delete_range(crate::core::Range::new(below_start, below_end));
                buf.insert_str(crate::core::Position::new(line + 1, 0), &format!("{}\n", below_text));
                let insert_pos = crate::core::Position::new(line, 0);
                buf.insert_str(insert_pos, &format!("{}\n", line_text));
                buf.cursor.line = (line + 1).min(buf.line_count().saturating_sub(1));
                let new_line_len = buf.chars_at_line(buf.cursor.line);
                buf.cursor.column = buf.cursor.column.min(new_line_len);
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::ToggleComment => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                let line_text = buf.get_line(line);
                if line_text.trim_start().starts_with("//") {
                    let leading = line_text.len() - line_text.trim_start().len();
                    let range = crate::core::Range::new(
                        crate::core::Position::new(line, leading),
                        crate::core::Position::new(line, leading + 2),
                    );
                    buf.delete_range(range);
                    if buf.cursor.line == line && buf.cursor.column >= leading + 2 {
                        buf.cursor.column = buf.cursor.column.saturating_sub(2);
                        ctx.editor.cursor.position = buf.cursor;
                    }
                } else {
                    buf.insert_str(crate::core::Position::new(line, 0), "//");
                    if buf.cursor.line == line {
                        buf.cursor.column += 2;
                        ctx.editor.cursor.position = buf.cursor;
                    }
                }
                buf.set_modified();
            }
            Action::SwitchToInsertMode => {
                ctx.editor.editor_mode.switch_to_insert();
            }
            Action::SwitchToNormalMode => {
                ctx.editor.editor_mode.switch_to_normal();
                ctx.editor.selection.clear();
            }
            Action::SwitchToVisualMode => {
                ctx.editor.editor_mode.switch_to_visual();
                ctx.editor.selection.start(ctx.editor.cursor.position);
            }
            Action::SwitchToVisualLineMode => {
                let pos = ctx.editor.cursor.position;
                let start = crate::core::Position::new(pos.line, 0);
                ctx.editor.selection.start(start);
                ctx.editor.editor_mode.set(crate::core::EditMode::VisualLine);
            }
            Action::SwitchToCommandMode => {
                ctx.editor.editor_mode.switch_to_command();
            }
            Action::SwitchToSearchMode => {
                ctx.editor.editor_mode.set(crate::core::EditMode::Search);
            }
            Action::SelectAll => {
                let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let last_line = buf.line_count().saturating_sub(1);
                let last_col = buf.chars_at_line(last_line);
                let start = crate::core::Position::zero();
                let end = crate::core::Position::new(last_line, last_col);
                ctx.editor.selection.select_all(start, end);
            }
            Action::SelectLine => {
                let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line = buf.cursor.line;
                let start = crate::core::Position::new(line, 0);
                let line_len = buf.chars_at_line(line);
                let end = crate::core::Position::new(line, line_len);
                ctx.editor.selection.select_all(start, end);
            }
            Action::SelectToMatchingBrace => {
                let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let pos = buf.cursor;
                if let Some(match_pos) = find_matching_brace_char(buf, pos) {
                    ctx.editor.selection.select_all(pos, match_pos);
                }
            }
            Action::ExpandSelection => {
                if let Some(range) = ctx.editor.selection.normalized_range() {
                    let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                    let mut new_start = range.start;
                    let mut new_end = range.end;
                    if new_start.column > 0 {
                        new_start.column -= 1;
                    } else if new_start.line > 0 {
                        new_start.line -= 1;
                        new_start.column = buf.chars_at_line(new_start.line);
                    }
                    if new_end.column < buf.chars_at_line(new_end.line) {
                        new_end.column += 1;
                    } else if new_end.line + 1 < buf.line_count() {
                        new_end.line += 1;
                        new_end.column = 0;
                    }
                    ctx.editor.selection.select_all(new_start, new_end);
                }
            }
            Action::ShrinkSelection => {
                if let Some(range) = ctx.editor.selection.normalized_range() {
                    let mut new_start = range.start;
                    let mut new_end = range.end;
                    if new_start.column < new_end.column || new_start.line < new_end.line {
                        if new_start.column < ::std::usize::MAX {
                            new_start.column += 1;
                        }
                        if new_end.column > 0 {
                            new_end.column -= 1;
                        } else if new_end.line > 0 {
                            new_end.line -= 1;
                            let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                            new_end.column = buf.chars_at_line(new_end.line);
                        }
                    }
                    if new_start <= new_end {
                        ctx.editor.selection.select_all(new_start, new_end);
                    }
                }
            }
            Action::Copy => {
                if let Some(range) = ctx.editor.selection.normalized_range() {
                    let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                    let text = buf.get_text_in_range(range);
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(text);
                    }
                }
            }
            Action::Cut => {
                if let Some(range) = ctx.editor.selection.normalized_range() {
                    let text;
                    {
                        let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                        text = buf.get_text_in_range(range);
                    }
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(text);
                    }
                    let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                    buf.delete_range(range);
                    buf.cursor = range.start;
                    ctx.editor.cursor.position = range.start;
                    buf.set_modified();
                }
                ctx.editor.selection.clear();
            }
            Action::Paste => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        let cleaned = text.replace("\r\n", "\n").replace('\r', "\n");
                        if ctx.editor.selection.is_active && !ctx.editor.selection.is_empty() {
                            if let Some(range) = ctx.editor.selection.normalized_range() {
                                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                                buf.delete_range(range);
                                ctx.editor.cursor.position = range.start;
                            }
                            ctx.editor.selection.clear();
                        }
                        let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                        let pos = ctx.editor.cursor.position;
                        buf.insert_str(pos, &cleaned);
                        let newlines = cleaned.chars().filter(|&c| c == '\n').count();
                        let new_pos = if newlines == 0 {
                            crate::core::Position::new(pos.line, pos.column + cleaned.chars().count())
                        } else {
                            crate::core::Position::new(pos.line + newlines, cleaned.split('\n').last().unwrap_or("").chars().count())
                        };
                        buf.cursor = new_pos;
                        ctx.editor.cursor.position = new_pos;
                        ctx.editor.cursor.preferred_column = new_pos.column;
                        buf.set_modified();
                    }
                }
            }
            Action::CopyLine => {
                let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let line_text = buf.get_line(buf.cursor.line);
                let text = format!("{}\n", line_text);
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(text);
                }
            }
            Action::CutLine => {
                let line_text;
                let line;
                {
                    let buf = ctx.editor.buffers.get(ctx.editor.active_buffer).ok_or("No active buffer")?;
                    line = buf.cursor.line;
                    line_text = buf.get_line(line);
                }
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(line_text);
                }
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                let start = crate::core::Position::new(line, 0);
                let end = if line + 1 < buf.line_count() {
                    crate::core::Position::new(line + 1, 0)
                } else {
                    crate::core::Position::new(line, buf.chars_at_line(line))
                };
                buf.delete_range(crate::core::Range::new(start, end));
                buf.cursor.line = buf.cursor.line.min(buf.line_count().saturating_sub(1));
                buf.cursor.column = 0;
                ctx.editor.cursor.position = buf.cursor;
                buf.set_modified();
            }
            Action::Undo => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                if let Some(entry) = ctx.editor.histories[ctx.editor.active_buffer].undo() {
                    for change in &entry.changes {
                        match change {
                            crate::editor::history::ChangeKind::Insert { ref pos, text } => {
                                let end_pos = crate::core::Position::new(pos.line, pos.column + text.chars().count());
                                let range = crate::core::Range::new(*pos, end_pos);
                                buf.delete_range(range);
                            }
                            crate::editor::history::ChangeKind::Delete { pos, text, range } => {
                                let r = *range;
                                buf.delete_range(r);
                                buf.insert_str(*pos, text.as_str());
                            }
                            crate::editor::history::ChangeKind::Replace { range, old, .. } => {
                                let norm = range.normalized();
                                buf.delete_range(norm);
                                buf.insert_str(norm.start, old.as_str());
                            }
                            crate::editor::history::ChangeKind::Indent { line } => {
                                let l = *line;
                                let line_str = buf.get_line(l);
                                let to_remove = line_str.chars().take_while(|c| *c == ' ').take(4).count();
                                if to_remove > 0 {
                                    buf.delete_range(crate::core::Range::new(
                                        crate::core::Position::new(l, 0),
                                        crate::core::Position::new(l, to_remove),
                                    ));
                                }
                            }
                            crate::editor::history::ChangeKind::Unindent { line } => {
                                buf.insert_str(crate::core::Position::new(*line, 0), "    ");
                            }
                        }
                    }
                    buf.cursor = entry.cursor_before;
                    ctx.editor.cursor.position = entry.cursor_before;
                    buf.set_modified();
                }
            }
            Action::Redo => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                if let Some(entry) = ctx.editor.histories[ctx.editor.active_buffer].redo() {
                    for change in &entry.changes {
                        match change {
                            crate::editor::history::ChangeKind::Insert { pos, text } => {
                                buf.insert_str(*pos, text.as_str());
                            }
                            crate::editor::history::ChangeKind::Delete { pos, range, .. } => {
                                let r = *range;
                                buf.delete_range(r);
                                buf.cursor = *pos;
                            }
                            crate::editor::history::ChangeKind::Replace { range, new, .. } => {
                                let norm = range.normalized();
                                buf.delete_range(norm);
                                buf.insert_str(norm.start, new.as_str());
                            }
                            crate::editor::history::ChangeKind::Indent { line } => {
                                buf.insert_str(crate::core::Position::new(*line, 0), "    ");
                            }
                            crate::editor::history::ChangeKind::Unindent { line } => {
                                let l = *line;
                                let line_str = buf.get_line(l);
                                let to_remove = line_str.chars().take_while(|c| *c == ' ').take(4).count();
                                if to_remove > 0 {
                                    buf.delete_range(crate::core::Range::new(
                                        crate::core::Position::new(l, 0),
                                        crate::core::Position::new(l, to_remove),
                                    ));
                                }
                            }
                        }
                    }
                    buf.cursor = entry.cursor_after;
                    ctx.editor.cursor.position = entry.cursor_after;
                    buf.set_modified();
                }
            }
            Action::OpenFile => {
                ctx.ui.notifications.push(crate::core::types::Notification::info(
                    "Use :e <path> to open a file",
                ));
            }
            Action::SaveFile => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                if buf.path.is_some() {
                    buf.save().map_err(|e| format!("Failed to save: {}", e))?;
                    ctx.ui.notifications.push(crate::core::types::Notification::success(
                        "File saved",
                    ));
                } else {
                    ctx.ui.notifications.push(crate::core::types::Notification::info(
                        "Use :w <path> to save as",
                    ));
                }
            }
            Action::SaveAs => {
                ctx.ui.notifications.push(crate::core::types::Notification::info(
                    "Save as not yet implemented",
                ));
            }
            Action::CloseFile => {
                if ctx.editor.buffers.len() > 1 {
                    if ctx.editor.buffers[ctx.editor.active_buffer].dirty {
                        return Err("Unsaved changes. Use :q! to force quit".to_string());
                    }
                    ctx.editor.buffers.remove(ctx.editor.active_buffer);
                    if ctx.editor.active_buffer >= ctx.editor.buffers.len() {
                        ctx.editor.active_buffer = ctx.editor.buffers.len().saturating_sub(1);
                    }
                    let buf = &ctx.editor.buffers[ctx.editor.active_buffer];
                    ctx.editor.cursor.position = buf.cursor;
                } else {
                    return Err("Cannot close the last buffer".to_string());
                }
            }
            Action::ReloadFile => {
                let buf = ctx.editor.buffers.get_mut(ctx.editor.active_buffer).ok_or("No active buffer")?;
                buf.load().map_err(|e| format!("Failed to reload: {}", e))?;
                ctx.ui.notifications.push(crate::core::types::Notification::success(
                    "File reloaded",
                ));
            }
            Action::NewFile => {
                let id = ctx.editor.buffers.len();
                let buf = crate::core::buffer::Buffer::new(id);
                ctx.editor.buffers.push(buf);
                ctx.editor.active_buffer = ctx.editor.buffers.len() - 1;
            }
            Action::SearchForward => {
                ctx.editor.editor_mode.set(crate::core::EditMode::Search);
            }
            Action::SearchBackward => {
                ctx.editor.editor_mode.set(crate::core::EditMode::Search);
            }
            Action::FindNext => {}
            Action::FindPrevious => {}
            Action::Replace => {}
            Action::ReplaceAll => {}
            Action::SearchToggleRegex => {}
            Action::SearchToggleCaseSensitive => {}
            Action::GoToLine(None) => {}
            Action::FuzzyFindFile => {}
            Action::FindSymbol => {}
            Action::FindHeading => {}
            Action::NextBuffer => {
                if ctx.editor.buffers.len() > 1 {
                    ctx.editor.active_buffer = (ctx.editor.active_buffer + 1) % ctx.editor.buffers.len();
                    let buf = &ctx.editor.buffers[ctx.editor.active_buffer];
                    ctx.editor.cursor.position = buf.cursor;
                }
            }
            Action::PreviousBuffer => {
                if ctx.editor.buffers.len() > 1 {
                    ctx.editor.active_buffer = if ctx.editor.active_buffer == 0 {
                        ctx.editor.buffers.len() - 1
                    } else {
                        ctx.editor.active_buffer - 1
                    };
                    let buf = &ctx.editor.buffers[ctx.editor.active_buffer];
                    ctx.editor.cursor.position = buf.cursor;
                }
            }
            Action::SwitchBuffer(n) => {
                if *n < ctx.editor.buffers.len() {
                    ctx.editor.active_buffer = *n;
                    let buf = &ctx.editor.buffers[ctx.editor.active_buffer];
                    ctx.editor.cursor.position = buf.cursor;
                }
            }
            Action::SplitHorizontal => {}
            Action::SplitVertical => {}
            Action::ClosePane => {}
            Action::NextSplit => {}
            Action::PreviousSplit => {}
            Action::ToggleMarkdownPreview => {}
            Action::ToggleSideBySide => {}
            Action::FocusEditor => {}
            Action::FocusPreview => {}
            Action::ToggleFileTree => {}
            Action::FocusFileTree => {}
            Action::ToggleWorkspaceSearch => {}
            Action::FocusWorkspaceSearch => {}
            Action::ShowPalette => {}
            Action::ShowNotifications => {}
            Action::ToggleStatusBar => {}
            Action::ToggleLineNumbers => {}
            Action::ToggleRelativeLineNumbers => {}
            Action::ToggleWordWrap => {}
            Action::IncreaseFontSize => {}
            Action::DecreaseFontSize => {}
            Action::ResetFontSize => {}
            Action::Quit => {
                let has_dirty = ctx.editor.buffers.iter().any(|b| b.dirty);
                if has_dirty && !ctx.app.force_quit {
                    return Err("Unsaved changes. Use :q! to force quit".to_string());
                }
                ctx.app.quit_requested = true;
            }
            Action::ForceQuit => {
                ctx.app.force_quit = true;
                ctx.app.quit_requested = true;
            }
            Action::Suspend => {}
            Action::DebugInfo => {}
            Action::ToggleLogPanel => {}
            Action::ReloadConfig => {}
            Action::RepeatLastAction => {}
            Action::MacroStart => {}
            Action::MacroEnd => {}
            Action::MacroPlay => {}
            Action::Noop => {}
            _ => {}
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredCommand> {
        let resolved = self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name);
        self.commands.get(resolved)
    }

    pub fn complete(&self, prefix: &str) -> Vec<String> {
        let mut matches = Vec::new();
        for name in self.commands.keys() {
            if name.starts_with(prefix) {
                matches.push(name.clone());
            }
        }
        for alias in self.aliases.keys() {
            if alias.starts_with(prefix) && !matches.contains(alias) {
                matches.push(alias.clone());
            }
        }
        matches.sort();
        matches
    }

    pub fn all_commands(&self) -> Vec<&RegisteredCommand> {
        let mut cmds: Vec<&RegisteredCommand> = self.commands.values().collect();
        cmds.sort_by(|a, b| a.name.cmp(&b.name));
        cmds
    }

    pub fn all_actions(&self) -> Vec<String> {
        let mut names: Vec<String> = self.commands.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn execute_command_internal(
    registry: &CommandRegistry,
    name: &str,
    args: &[String],
    ctx: &mut crate::app::AppContext,
) -> Result<(), String> {
    registry.execute(name, args, ctx)
}
