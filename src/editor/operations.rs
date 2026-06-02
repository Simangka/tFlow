use crate::core::{Position, Range, Movement};
use crate::core::buffer::Buffer;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;
use crate::editor::history::ChangeKind;

const INDENT_WIDTH: usize = 4;

fn line_chars_excl_newline(buf: &Buffer, line: usize) -> usize {
    let total = buf.chars_at_line(line);
    if total > 0 && buf.get_line(line).ends_with('\n') { total - 1 } else { total }
}

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
        let total_lines = buffer.line_count();
        if pos.column >= line_len.saturating_sub(1) && pos.line + 1 < total_lines {
            let del_pos = Position::new(pos.line, line_len.saturating_sub(1));
            let c = buffer.delete_char(del_pos).ok_or(())?;
            buffer.set_modified();
            let range = Range::new(del_pos, Position::new(del_pos.line, del_pos.column + 1));
            Ok(Some(ChangeKind::Delete {
                pos: del_pos,
                text: c.to_string(),
                range,
            }))
        } else if pos.column < line_len {
            let del_pos = pos;
            let c = buffer.delete_char(del_pos).ok_or(())?;
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
        let total_lines = buffer.line_count();
        let use_tabs = (0..total_lines).any(|l| buffer.get_line(l).starts_with('\t'));
        let (indent_str, indent_width) = if use_tabs { ("\t", 1) } else { ("    ", INDENT_WIDTH) };
        buffer.insert_str(pos, indent_str);
        if cursor.position.line == pos.line {
            cursor.position.column += indent_width;
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
        let to_remove = if line_text.starts_with('\t') {
            1
        } else {
            line_text.chars().take_while(|c| *c == ' ').take(INDENT_WIDTH).count()
        };
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
        let line_len = buffer.chars_at_line(line);
        let _ = buffer.delete_range(Range::new(
            Position::new(line, 0),
            Position::new(line, line_len),
        ));
        let _total_after_first = buffer.line_count();
        let above_len = buffer.chars_at_line(line - 1);
        let _ = buffer.delete_range(Range::new(
            Position::new(line - 1, 0),
            Position::new(line - 1, above_len),
        ));
        let _total_after_second = buffer.line_count();
        let insert_above = Position::new(line - 1, 0);
        buffer.insert_str(insert_above, &format!("{}\n", line_text));
        let _total_after_first_insert = buffer.line_count();
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
        let line_len = buffer.chars_at_line(line);
        let _ = buffer.delete_range(Range::new(
            Position::new(line, 0),
            Position::new(line, line_len),
        ));
        let _total_after_first = buffer.line_count();
        let below_len = buffer.chars_at_line(line);
        let _ = buffer.delete_range(Range::new(
            Position::new(line, 0),
            Position::new(line, below_len),
        ));
        let _total_after_second = buffer.line_count();
        let insert_below = Position::new(line, 0);
        buffer.insert_str(insert_below, &format!("{}\n", below_text));
        let _total_after_first_insert = buffer.line_count();
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
        let next_line_text_raw = buffer.get_line(line + 1);
        let leading_spaces = next_line_text_raw.chars().take_while(|c| *c == ' ').count();
        let next_line_text_trimmed: String = next_line_text_raw.chars().skip(leading_spaces).collect();
        let prev_line_raw = buffer.get_line(line);
        let prev_trimmed = prev_line_raw.strip_suffix('\n').unwrap_or(&prev_line_raw);
        let prev_last = prev_trimmed.chars().last();
        let next_first = next_line_text_trimmed.chars().next();
        let prev_ok = match prev_last {
            Some(c) => !c.is_whitespace(),
            None => false,
        };
        let next_ok = match next_first {
            Some(c) => !c.is_whitespace() && !c.is_ascii_punctuation(),
            None => false,
        };
        let needs_space = prev_ok && next_ok;
        let to_delete_len = 1 + leading_spaces;
        let _ = buffer.delete_range(Range::new(
            Position::new(line, current_line_len),
            Position::new(line, current_line_len + to_delete_len),
        ));
        if needs_space {
            buffer.insert_str(Position::new(line, current_line_len), " ");
        }
        let new_col = if needs_space {
            current_line_len + 1
        } else {
            current_line_len
        };
        cursor.position = Position::new(line, new_col);
        cursor.preferred_column = new_col;
        buffer.set_modified();
        let new_len = if needs_space {
            current_line_len + 1 + next_line_text_trimmed.chars().count()
        } else {
            current_line_len + next_line_text_trimmed.chars().count()
        };
        Ok(ChangeKind::Replace {
            range: Range::new(
                Position::new(line, current_line_len),
                Position::new(line, new_len),
            ),
            old: "\n".to_string(),
            new: if needs_space { " ".to_string() } else { String::new() },
        })
    }

    pub fn toggle_comment(buffer: &mut Buffer, cursor: &mut Cursor) -> Result<Option<ChangeKind>, ()> {
        let line = cursor.position.line;
        let line_text = buffer.get_line(line);
        let trimmed = line_text.trim_start();
        if trimmed.starts_with("//") {
            let leading_spaces = line_text.chars().take_while(|c| c.is_whitespace()).count();
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
            let total = buffer.chars_at_line(l);
            let excl_nl = if total > 0 && buffer.get_line(l).ends_with('\n') { total - 1 } else { total };
            if c < excl_nl {
                return buffer.char_at(Position::new(l, c));
            }
            if total > 0 && c == total - 1 && buffer.get_line(l).ends_with('\n') {
                return Some('\n');
            }
            None
        };

        let is_word_char = |c: char| -> bool {
            c.is_alphanumeric() || c == '_'
        };

        let is_whitespace = |c: char| -> bool {
            c == ' ' || c == '\t' || c == '\n'
        };

        let current = get_char(line, col);
        match current {
            Some(c) if is_whitespace(c) => {
                while let Some(c) = get_char(line, col) {
                    if !is_whitespace(c) {
                        break;
                    }
                    if c == '\n' {
                        if line + 1 >= total_lines {
                            return Position::new(line, col);
                        }
                        line += 1;
                        col = 0;
                    } else {
                        col += 1;
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
                    if let Some(nl) = get_char(line, col) {
                        if nl == '\n' {
                            if line + 1 >= total_lines {
                                return Position::new(line, col);
                            }
                            line += 1;
                            col = 0;
                        }
                    } else {
                        if line + 1 >= total_lines {
                            return Position::new(total_lines - 1, line_chars_excl_newline(buffer, total_lines - 1));
                        }
                        line += 1;
                        col = 0;
                    }
                }
                while let Some(c) = get_char(line, col) {
                    if !is_whitespace(c) {
                        break;
                    }
                    if c == '\n' {
                        if line + 1 >= total_lines {
                            return Position::new(line, col);
                        }
                        line += 1;
                        col = 0;
                    } else {
                        col += 1;
                    }
                }
                Position::new(line, col)
            }
            Some(_) => {
                col += 1;
                if let Some(nl) = get_char(line, col) {
                    if nl == '\n' {
                        if line + 1 >= total_lines {
                            return Position::new(line, col);
                        }
                        line += 1;
                        col = 0;
                    }
                } else {
                    if line + 1 >= total_lines {
                        return Position::new(total_lines - 1, line_chars_excl_newline(buffer, total_lines - 1));
                    }
                    line += 1;
                    col = 0;
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
            let total = buffer.chars_at_line(l);
            let excl_nl = if total > 0 && buffer.get_line(l).ends_with('\n') { total - 1 } else { total };
            if c < excl_nl {
                return buffer.char_at(Position::new(l, c));
            }
            if total > 0 && c == total - 1 && buffer.get_line(l).ends_with('\n') {
                return Some('\n');
            }
            None
        };

        let is_word_char = |c: char| -> bool {
            c.is_alphanumeric() || c == '_'
        };

        let is_whitespace = |c: char| -> bool {
            c == ' ' || c == '\t' || c == '\n'
        };

        let max_col = line_chars_excl_newline(buffer, line);
        if col > max_col {
            col = max_col;
        }

        if col == 0 {
            if line == 0 {
                return Position::new(0, 0);
            }
            line -= 1;
            col = line_chars_excl_newline(buffer, line);
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
                    if c == '\n' {
                        if line == 0 {
                            return Position::new(0, 0);
                        }
                        line -= 1;
                        col = line_chars_excl_newline(buffer, line);
                    } else if col == 0 {
                        if line == 0 {
                            return Position::new(0, 0);
                        }
                        line -= 1;
                        col = line_chars_excl_newline(buffer, line);
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
        let mut in_string: Option<char> = None;
        loop {
            if line >= total_lines {
                return None;
            }
            let line_len = buffer.chars_at_line(line);
            if col >= line_len {
                line += 1;
                col = 0;
                in_string = None;
                continue;
            }
            let c = buffer.char_at(Position::new(line, col))?;
            match in_string {
                Some(q) if c == q => {
                    in_string = None;
                }
                Some(_) => {}
                None => {
                    if c == '"' || c == '\'' {
                        in_string = Some(c);
                    } else if c == '\\' {
                        col += 1;
                    } else if c == open {
                        depth += 1;
                    } else if c == close {
                        depth -= 1;
                        if depth == 0 {
                            return Some(Position::new(line, col));
                        }
                    }
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
        let mut in_string: Option<char> = None;
        let mut prev_was_escape = false;
        loop {
            if !prev_was_escape {
                let c = buffer.char_at(Position::new(line, col))?;
                match in_string {
                    Some(q) if c == q => {
                        in_string = None;
                    }
                    Some(_) => {}
                    None => {
                        if c == '"' || c == '\'' {
                            in_string = Some(c);
                        } else if c == open {
                            depth -= 1;
                            if depth == 0 {
                                return Some(Position::new(line, col));
                            }
                        } else if c == close {
                            depth += 1;
                        }
                    }
                }
                prev_was_escape = c == '\\';
            } else {
                prev_was_escape = false;
            }
            if col == 0 {
                if line == 0 {
                    return None;
                }
                line -= 1;
                col = buffer.chars_at_line(line);
                in_string = None;
                prev_was_escape = false;
            } else {
                col -= 1;
            }
        }
    }
}
