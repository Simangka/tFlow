use crate::core::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn from_single_line(line: usize, start_col: usize, end_col: usize) -> Self {
        Self {
            start: Position::new(line, start_col),
            end: Position::new(line, end_col),
        }
    }

    pub fn normalized(&self) -> Self {
        if self.start <= self.end {
            *self
        } else {
            Self {
                start: self.end,
                end: self.start,
            }
        }
    }

    pub fn contains(&self, pos: Position) -> bool {
        let norm = self.normalized();
        pos >= norm.start && pos <= norm.end
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn lines(&self) -> usize {
        self.start.line.abs_diff(self.end.line) + 1
    }
}
