use crate::core::Position;
use crate::core::buffer::Buffer;

#[derive(Debug, Clone)]
pub struct Cursor {
    pub position: Position,
    pub preferred_column: usize,
    pub blink_state: bool,
    pub blink_timer: std::time::Instant,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            position: Position::zero(),
            preferred_column: 0,
            blink_state: true,
            blink_timer: std::time::Instant::now(),
        }
    }

    pub fn move_to(&mut self, line: usize, col: usize) {
        self.position = Position::new(line, col);
        self.preferred_column = col;
    }

    pub fn move_left(&mut self) -> Position {
        if self.position.column > 0 {
            self.position.column -= 1;
        } else if self.position.line > 0 {
            self.position.line -= 1;
        }
        self.preferred_column = self.position.column;
        self.position
    }

    pub fn move_right(&mut self, max_col: usize) -> Position {
        if self.position.column < max_col {
            self.position.column += 1;
        } else {
            self.position.line += 1;
            self.position.column = 0;
        }
        self.preferred_column = self.position.column;
        self.position
    }

    pub fn move_up(&mut self, above_line_len: usize) -> Position {
        if self.position.line > 0 {
            self.position.line -= 1;
        }
        self.position.column = self.preferred_column.min(above_line_len);
        self.position
    }

    pub fn move_down(&mut self, below_line_len: usize) -> Position {
        self.position.line += 1;
        self.position.column = self.preferred_column.min(below_line_len);
        self.position
    }

    pub fn start_of_line(&mut self) {
        self.position.column = 0;
        self.preferred_column = 0;
    }

    pub fn end_of_line(&mut self, line_len: usize) {
        self.position.column = line_len;
        self.preferred_column = line_len;
    }

    pub fn reset_blink(&mut self) {
        self.blink_state = true;
        self.blink_timer = std::time::Instant::now();
    }

    pub fn toggle_blink(&mut self) {
        self.blink_state = !self.blink_state;
    }

    pub fn move_left_in(&mut self, buf: &Buffer) -> Position {
        if self.position.column > 0 {
            self.position.column -= 1;
        } else if self.position.line > 0 {
            self.position.line -= 1;
            self.position.column = buf.chars_at_line(self.position.line).saturating_sub(1);
        }
        self.preferred_column = self.position.column;
        self.position
    }

    pub fn move_right_in(&mut self, buf: &Buffer) -> Position {
        let line_max = buf.chars_at_line(self.position.line).saturating_sub(1);
        if self.position.column < line_max {
            self.position.column += 1;
        } else if self.position.line + 1 < buf.line_count() {
            self.position.line += 1;
            self.position.column = 0;
        }
        self.preferred_column = self.position.column;
        self.position
    }

    pub fn move_up_in(&mut self, buf: &Buffer) -> Position {
        if self.position.line > 0 {
            self.position.line -= 1;
        }
        let line_max = buf.chars_at_line(self.position.line).saturating_sub(1);
        self.position.column = self.preferred_column.min(line_max);
        self.position
    }

    pub fn move_down_in(&mut self, buf: &Buffer) -> Position {
        let last = buf.line_count().saturating_sub(1);
        if self.position.line < last {
            self.position.line += 1;
        }
        let line_max = buf.chars_at_line(self.position.line).saturating_sub(1);
        self.position.column = self.preferred_column.min(line_max);
        self.position
    }

    pub fn set_position_clamped(&mut self, pos: Position, buf: &Buffer) {
        self.position = buf.clamp_position(pos);
        self.preferred_column = self.position.column;
    }

    pub fn previous_word_boundary(&self, _buf: &Buffer) -> Position {
        self.position
    }
}
