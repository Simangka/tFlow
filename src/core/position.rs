use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    pub fn new_checked(line: usize, column: usize, max_line: usize, max_col: usize) -> Option<Self> {
        if line <= max_line && column <= max_col {
            Some(Self { line, column })
        } else {
            None
        }
    }

    pub fn zero() -> Self {
        Self { line: 0, column: 0 }
    }

    pub fn saturating_sub(&self, other: &Self) -> Self {
        if self.line < other.line {
            return Self { line: 0, column: 0 };
        }
        let line = self.line - other.line;
        let column = if self.line == other.line {
            self.column.saturating_sub(other.column)
        } else {
            self.column
        };
        Self { line, column }
    }

    pub fn min(&self, other: &Self) -> Self {
        if self <= other {
            *self
        } else {
            *other
        }
    }

    pub fn max(&self, other: &Self) -> Self {
        if self >= other {
            *self
        } else {
            *other
        }
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        self.line
            .cmp(&other.line)
            .then(self.column.cmp(&other.column))
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.column + 1)
    }
}
