use crate::core::{Position, Range, Movement};
use crate::core::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::editor::history::ChangeKind;

pub struct EditOperations;

impl EditOperations {
    pub fn insert_char(buffer: &mut Buffer, cursor: &mut Cursor, c: char) -> Result<ChangeKind, ()> {
        let pos = cursor.position;
        buffer.insert_char(pos, c);
        if c == '\n' {
            cursor.position = Position::new(pos.line + 1, 0);
        } else {
            cursor.position = Position::new(pos.line, pos.column + 1);
        }
        cursor.preferred_column = cursor.position.column;
        buffer.set_modified();
        Ok(ChangeKind::Insert {
            pos,
            text: c.to_string(),
        })
    }

    pub fn insert_newline(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<ChangeKind, ()> {
        let pos = cursor.position;
        buffer.insert_newline(pos);
        cursor.position = Position::new(pos.line + 1, 0);
        cursor.preferred_column = 0;
        buffer.set_modified();
        Ok(ChangeKind::Insert {
            pos,
            text: "\n".to_string(),
        })
    }

    pub fn delete_char_forward(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<Option<ChangeKind>, ()> {
        let pos = cursor.position;
        let line_len = buffer.chars_at_line(pos.line);
        if pos.column < line_len || pos.line + 1 < buffer.line_count() {
            let (del_pos, c) = if pos.column < line_len {
                (pos, buffer.char_at(pos).ok_or(())?)
            } else {
                let next_pos = Position::new(pos.line + 1, 0);
                (next_pos, '\n')
            };
            buffer.delete_char(del_pos).ok_or(())?;
            buffer.set_modified();
            let range = Range::new(del_pos, Position::new(del_pos.line, del_pos.column + 1));
            Ok(Some(ChangeKind::Delete {
                pos: del_pos,
                text: c.to_string(),
                range,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_char_backward(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<Option<ChangeKind>, ()> {
        if cursor.position.column == 0 && cursor.position.line == 0 {
            return Ok(None);
        }
        let total_lines = buffer.line_count();
        if total_lines == 0 {
            return Ok(None);
        }
        let line = cursor.position.line.min(total_lines.saturating_sub(1));
        let line_len = buffer.chars_at_line(line);
        let col = cursor.position.column.min(line_len);
        if col == 0 {
            if line == 0 {
                return Ok(None);
            }
            let prev_line = line - 1;
            let prev_line_len = buffer.chars_at_line(prev_line);
            let p = Position::new(prev_line, prev_line_len);
            if buffer.delete_char(p).is_none() {
                return Ok(None);
            }
            cursor.position = Position::new(prev_line, prev_line_len);
            cursor.preferred_column = prev_line_len;
            buffer.set_modified();
            return Ok(Some(ChangeKind::Delete {
                pos: p, text: "\n".to_string(),
                range: Range::new(p, Position::new(prev_line, prev_line_len + 1)),
            }));
        }
        let del_col = col - 1;
        let p = Position::new(line, del_col);
        let ch = match buffer.char_at(p) {
            Some(c) => c,
            None => return Ok(None),
        };
        if buffer.delete_char(p).is_none() {
            return Ok(None);
        }
        cursor.position = Position::new(line, del_col);
        cursor.preferred_column = del_col;
        buffer.set_modified();
        Ok(Some(ChangeKind::Delete {
            pos: p, text: ch.to_string(),
            range: Range::new(p, Position::new(line, del_col + 1)),
        }))
    }

    pub fn delete_selection(buffer: &mut Buffer, sel: &Selection) -> Result<Option<ChangeKind>, ()> {
        let range = match sel.normalized_range() {
            Some(r) if !r.is_empty() => r,
            _ => return Ok(None),
        };
        let deleted = buffer.delete_range(range);
        buffer.set_modified();
        Ok(Some(ChangeKind::Delete {
            pos: range.start,
            text: deleted.clone(),
            range,
        }))
    }

    pub fn indent_line(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<ChangeKind, ()> {
        let pos = Position::new(cursor.position.line, 0);
        buffer.insert_str(pos, "    ");
        if cursor.position.line == pos.line {
            cursor.position.column += 4;
            cursor.preferred_column = cursor.position.column;
        }
        buffer.set_modified();
        Ok(ChangeKind::Indent {
            line: cursor.position.line,
        })
    }

    pub fn unindent_line(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<ChangeKind, ()> {
        let line = cursor.position.line;
        let line_text = buffer.get_line(line);
        let to_remove = line_text.chars().take_while(|c| *c == ' ').take(4).count();
        if to_remove == 0 {
            return Err(());
        }
        let start = Position::new(line, 0);
        let end = Position::new(line, to_remove);
        buffer.delete_range(Range::new(start, end));
        if cursor.position.line == line {
            cursor.position.column = cursor.position.column.saturating_sub(to_remove);
            cursor.preferred_column = cursor.position.column;
        }
        buffer.set_modified();
        Ok(ChangeKind::Unindent { line })
    }

    pub fn duplicate_line(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<ChangeKind, ()> {
        let line = cursor.position.line;
        let line_text = buffer.get_line(line);
        let insert_pos = Position::new(line + 1, 0);
        let text = format!("{}\n", line_text);
        buffer.insert_str(insert_pos, &text);
        cursor.position = Position::new(line + 1, cursor.position.column);
        cursor.preferred_column = cursor.position.column;
        buffer.set_modified();
        Ok(ChangeKind::Insert {
            pos: insert_pos,
            text,
        })
    }

    pub fn move_line_up(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<(), ()> {
        let line = cursor.position.line;
        if line == 0 {
            return Err(());
        }
        let line_text = buffer.get_line(line);
        let above_text = buffer.get_line(line - 1);
        let line_range = Range::new(
            Position::new(line, 0),
            Position::new(line, buffer.chars_at_line(line)),
        );
        let above_range = Range::new(
            Position::new(line - 1, 0),
            Position::new(line - 1, buffer.chars_at_line(line - 1)),
        );
        let _ = buffer.delete_range(line_range);
        let _ = buffer.delete_range(above_range);
        let insert_above = Position::new(line - 1, 0);
        buffer.insert_str(insert_above, &format!("{}\n", line_text));
        let insert_below = Position::new(line, 0);
        buffer.insert_str(insert_below, &format!("{}\n", above_text));
        buffer.set_modified();
        cursor.position.line = line - 1;
        Ok(())
    }

    pub fn move_line_down(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<(), ()> {
        let line = cursor.position.line;
        if line + 1 >= buffer.line_count() {
            return Err(());
        }
        let line_text = buffer.get_line(line);
        let below_text = buffer.get_line(line + 1);
        let line_range = Range::new(
            Position::new(line, 0),
            Position::new(line, buffer.chars_at_line(line)),
        );
        let below_range = Range::new(
            Position::new(line + 1, 0),
            Position::new(line + 1, buffer.chars_at_line(line + 1)),
        );
        let _ = buffer.delete_range(line_range);
        let _ = buffer.delete_range(below_range);
        let insert_pos = Position::new(line + 1, 0);
        buffer.insert_str(insert_pos, &format!("{}\n", below_text));
        let insert_pos2 = Position::new(line, 0);
        buffer.insert_str(insert_pos2, &format!("{}\n", line_text));
        buffer.set_modified();
        cursor.position.line = line + 1;
        Ok(())
    }

    pub fn join_lines(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<ChangeKind, ()> {
        let line = cursor.position.line;
        if line + 1 >= buffer.line_count() {
            return Err(());
        }
        let current_line_len = buffer.chars_at_line(line);
        let newline_pos = Position::new(line, current_line_len);
        let mut next_line_text = buffer.get_line(line + 1);
        let leading_spaces = next_line_text.chars().take_while(|c| *c == ' ').count();
        if leading_spaces > 0 {
            next_line_text = next_line_text.chars().skip(leading_spaces).collect();
        }
        if current_line_len > 0 && !buffer.get_line(line).ends_with(' ') {
            next_line_text.insert(0, ' ');
        }
        let _ = buffer.delete_char(newline_pos).ok_or(())?;
        let _ = buffer.delete_range(Range::new(
            Position::new(line + 1, 0),
            Position::new(line + 1, leading_spaces),
        ));
        buffer.insert_str(Position::new(line, buffer.chars_at_line(line)), &next_line_text);
        cursor.position = Position::new(line, current_line_len);
        cursor.preferred_column = cursor.position.column;
        buffer.set_modified();
        Ok(ChangeKind::Replace {
            range: Range::new(
                Position::new(line, current_line_len),
                Position::new(line + 1, next_line_text.len()),
            ),
            old: "\n".to_string(),
            new: " ".to_string(),
        })
    }

    pub fn toggle_comment(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<Option<ChangeKind>, ()> {
        let line = cursor.position.line;
        let line_text = buffer.get_line(line);
        let trimmed = line_text.trim_start();
        if trimmed.starts_with("//") {
            let leading_spaces = line_text.len() - line_text.trim_start().len();
            let start = Position::new(line, leading_spaces);
            let end = Position::new(line, leading_spaces + 2);
            buffer.delete_range(Range::new(start, end));
            buffer.set_modified();
            Ok(Some(ChangeKind::Unindent { line }))
        } else {
            let pos = Position::new(line, 0);
            buffer.insert_str(pos, "//");
            if cursor.position.line == line {
                cursor.position.column += 2;
                cursor.preferred_column = cursor.position.column;
            }
            buffer.set_modified();
            Ok(Some(ChangeKind::Indent { line }))
        }
    }

    pub fn apply_movement(buffer: &Buffer, cursor: &mut Cursor, movement: Movement) {
        match movement {
            Movement::Left => {
                cursor.move_left();
            }
            Movement::Right => {
                let max_col = buffer.chars_at_line(cursor.position.line);
                cursor.move_right(max_col);
            }
            Movement::Up => {
                let above_line_len = if cursor.position.line > 0 {
                    buffer.chars_at_line(cursor.position.line - 1)
                } else {
                    0
                };
                cursor.move_up(above_line_len);
            }
            Movement::Down => {
                let below_line_len = if cursor.position.line + 1 < buffer.line_count() {
                    buffer.chars_at_line(cursor.position.line + 1)
                } else {
                    buffer.chars_at_line(cursor.position.line)
                };
                cursor.move_down(below_line_len);
            }
            Movement::StartOfLine | Movement::LineStart => {
                cursor.start_of_line();
            }
            Movement::EndOfLine | Movement::LineEnd => {
                let line_len = buffer.chars_at_line(cursor.position.line);
                cursor.end_of_line(line_len);
            }
            Movement::StartOfFile => {
                cursor.move_to(0, 0);
            }
            Movement::EndOfFile => {
                let last_line = buffer.line_count().saturating_sub(1);
                let line_len = buffer.chars_at_line(last_line);
                cursor.move_to(last_line, line_len);
            }
            Movement::PageUp => {
                let target_line = cursor.position.line.saturating_sub(50);
                let line_len = buffer.chars_at_line(target_line);
                cursor.move_to(target_line, cursor.preferred_column.min(line_len));
            }
            Movement::PageDown => {
                let target_line = (cursor.position.line + 50).min(buffer.line_count().saturating_sub(1));
                let line_len = buffer.chars_at_line(target_line);
                cursor.move_to(target_line, cursor.preferred_column.min(line_len));
            }
            Movement::HalfPageUp => {
                let target_line = cursor.position.line.saturating_sub(25);
                let line_len = buffer.chars_at_line(target_line);
                cursor.move_to(target_line, cursor.preferred_column.min(line_len));
            }
            Movement::HalfPageDown => {
                let target_line = (cursor.position.line + 25).min(buffer.line_count().saturating_sub(1));
                let line_len = buffer.chars_at_line(target_line);
                cursor.move_to(target_line, cursor.preferred_column.min(line_len));
            }
            Movement::WordForward => {
                let new_pos = Self::word_forward(buffer, cursor.position);
                cursor.move_to(new_pos.line, new_pos.column);
            }
            Movement::WordBackward => {
                let new_pos = Self::word_backward(buffer, cursor.position);
                cursor.move_to(new_pos.line, new_pos.column);
            }
            Movement::MatchingBrace => {
                if let Some(match_pos) = Self::find_matching_brace(buffer, cursor.position) {
                    cursor.move_to(match_pos.line, match_pos.column);
                }
            }
        }
    }

    pub fn word_forward(buffer: &Buffer, pos: Position) -> Position {
        let total_lines = buffer.line_count();
        let total_chars = buffer.total_chars();
        if total_chars == 0 {
            return pos;
        }
        let mut line = pos.line;
        let mut col = pos.column;

        let get_char = |l: usize, c: usize| -> Option<char> {
            if l >= total_lines {
                return None;
            }
            let line_len = buffer.chars_at_line(l);
            if c >= line_len {
                return None;
            }
            buffer.char_at(Position::new(l, c))
        };

        let is_word_char = |c: char| -> bool {
            c.is_alphanumeric() || c == '_'
        };

        let is_whitespace = |c: char| -> bool {
            c == ' ' || c == '\t'
        };

        let current = get_char(line, col);
        match current {
            Some(c) if is_whitespace(c) => {
                while let Some(c) = get_char(line, col) {
                    if !is_whitespace(c) {
                        break;
                    }
                    col += 1;
                    if col > buffer.chars_at_line(line) {
                        col = 0;
                        line += 1;
                        if line >= total_lines {
                            return pos;
                        }
                    }
                }
                Position::new(line, col)
            }
            Some(c) if is_word_char(c) => {
                while let Some(c) = get_char(line, col) {
                    if !is_word_char(c) {
                        break;
                    }
                    col += 1;
                    if col > buffer.chars_at_line(line) {
                        col = 0;
                        line += 1;
                        if line >= total_lines {
                            return Position::new(total_lines - 1, buffer.chars_at_line(total_lines - 1));
                        }
                    }
                }
                while let Some(c) = get_char(line, col) {
                    if !is_whitespace(c) {
                        break;
                    }
                    col += 1;
                    if col > buffer.chars_at_line(line) {
                        col = 0;
                        line += 1;
                        if line >= total_lines {
                            return Position::new(total_lines - 1, buffer.chars_at_line(total_lines - 1));
                        }
                    }
                }
                Position::new(line, col)
            }
            Some(_) => {
                col += 1;
                if col > buffer.chars_at_line(line) {
                    col = 0;
                    line += 1;
                    if line >= total_lines {
                        return Position::new(total_lines - 1, buffer.chars_at_line(total_lines - 1));
                    }
                }
                Position::new(line, col)
            }
            None => pos,
        }
    }

    pub fn word_backward(buffer: &Buffer, pos: Position) -> Position {
        let total_lines = buffer.line_count();
        if total_lines == 0 {
            return pos;
        }
        let mut line = pos.line;
        let mut col = pos.column;

        let get_char = |l: usize, c: usize| -> Option<char> {
            if l >= total_lines {
                return None;
            }
            let line_len = buffer.chars_at_line(l);
            if c >= line_len {
                return None;
            }
            buffer.char_at(Position::new(l, c))
        };

        let is_word_char = |c: char| -> bool {
            c.is_alphanumeric() || c == '_'
        };

        let is_whitespace = |c: char| -> bool {
            c == ' ' || c == '\t'
        };

        if col > buffer.chars_at_line(line) {
            col = buffer.chars_at_line(line);
        }

        if col == 0 {
            if line == 0 {
                return Position::new(0, 0);
            }
            line -= 1;
            col = buffer.chars_at_line(line);
        } else {
            col -= 1;
        }

        let current = get_char(line, col);
        match current {
            Some(c) if is_whitespace(c) => {
                while let Some(c) = get_char(line, col) {
                    if !is_whitespace(c) {
                        break;
                    }
                    if col == 0 {
                        if line == 0 {
                            return Position::new(0, 0);
                        }
                        line -= 1;
                        col = buffer.chars_at_line(line);
                    } else {
                        col -= 1;
                    }
                }
                let c = get_char(line, col);
                match c {
                    Some(c) if is_word_char(c) => {
                        while col > 0 && get_char(line, col - 1).map_or(false, |pc| is_word_char(pc)) {
                            col -= 1;
                        }
                    }
                    Some(_) => {
                        if col > 0 {
                            col -= 1;
                        }
                    }
                    None => {}
                }
                Position::new(line, col)
            }
            Some(c) if is_word_char(c) => {
                while col > 0 && get_char(line, col - 1).map_or(false, |pc| is_word_char(pc)) {
                    col -= 1;
                }
                Position::new(line, col)
            }
            Some(_) => {
                Position::new(line, col)
            }
            None => pos,
        }
    }

    fn find_matching_brace(buffer: &Buffer, pos: Position) -> Option<Position> {
        let pairs = [('(', ')'), ('{', '}'), ('[', ']')];
        let c = buffer.char_at(pos)?;
        for &(open, close) in &pairs {
            if c == open {
                return Self::find_forward_match(buffer, pos, open, close);
            }
            if c == close {
                return Self::find_backward_match(buffer, pos, open, close);
            }
        }
        None
    }

    fn find_forward_match(buffer: &Buffer, pos: Position, open: char, close: char) -> Option<Position> {
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
            let c = buffer.char_at(Position::new(line, col))?;
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return Some(Position::new(line, col));
                }
            }
            col += 1;
        }
    }

    fn find_backward_match(buffer: &Buffer, pos: Position, open: char, close: char) -> Option<Position> {
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
            let c = buffer.char_at(Position::new(line, col))?;
            if c == close {
                depth += 1;
            } else if c == open {
                depth -= 1;
                if depth == 0 {
                    return Some(Position::new(line, col));
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
}
