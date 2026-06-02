use crate::core::{Position, Range, Direction};

#[derive(Debug, Clone)]
pub struct Selection {
    pub start: Option<Position>,
    pub end: Option<Position>,
    pub direction: Direction,
    pub is_active: bool,
}

impl Selection {
    pub fn new() -> Self {
        Self {
            start: None,
            end: None,
            direction: Direction::None,
            is_active: false,
        }
    }

    pub fn start(&mut self, pos: Position) {
        self.start = Some(pos);
        self.end = Some(pos);
        self.direction = Direction::None;
        self.is_active = true;
    }

    pub fn update(&mut self, pos: Position) {
        self.end = Some(pos);
        if let (Some(start), Some(end)) = (self.start, self.end) {
            self.direction = if end > start {
                Direction::Forward
            } else if end < start {
                Direction::Backward
            } else {
                Direction::None
            };
        }
        if self.start.is_some() {
            self.is_active = true;
        }
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
        self.direction = Direction::None;
        self.is_active = false;
    }

    pub fn range(&self) -> Option<Range> {
        match (self.start, self.end) {
            (Some(start), Some(end)) => Some(Range::new(start, end)),
            _ => None,
        }
    }

    pub fn normalized_range(&self) -> Option<Range> {
        self.range().map(|r| r.normalized())
    }

    pub fn is_empty(&self) -> bool {
        match (self.start, self.end) {
            (Some(start), Some(end)) => start == end,
            _ => true,
        }
    }

    pub fn toggle(&mut self, pos: Position) {
        if self.is_active {
            self.clear();
        } else {
            self.start(pos);
        }
    }

    pub fn select_all(&mut self, start: Position, end: Position) {
        self.start = Some(start);
        self.end = Some(end);
        self.direction = match end.cmp(&start) {
            std::cmp::Ordering::Greater => Direction::Forward,
            std::cmp::Ordering::Less => Direction::Backward,
            std::cmp::Ordering::Equal => Direction::None,
        };
        self.is_active = true;
    }

    pub fn extend_to_line(&mut self, pos: Position, line: usize) {
        let new_end = Position::new(line, pos.column);
        self.end = Some(new_end);
        if let Some(start) = self.start {
            self.direction = if new_end > start {
                Direction::Forward
            } else if new_end < start {
                Direction::Backward
            } else {
                Direction::None
            };
        }
        self.is_active = true;
    }
}
