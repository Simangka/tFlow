use ratatui::layout::{Layout, Constraint, Direction, Rect};

#[derive(Debug, Clone)]
pub struct UILayout {
    pub show_statusbar: bool,
    pub show_commandbar: bool,
    pub show_file_tree: bool,
    pub show_markdown_preview: bool,
    pub show_palette: bool,
    pub show_search_panel: bool,
    pub show_notifications: bool,
    pub split_direction: SplitDirection,
    pub split_ratio: f64,
    pub filetree_width: u16,
    pub preview_width_ratio: f64,
    pub preview_as_markdown: bool,
    pub focused_pane: FocusedPane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    Editor,
    FileTree,
}

#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub editor: Rect,
    pub gutter: Rect,
    pub filetree: Option<Rect>,
    pub markdown_preview: Option<Rect>,
    pub statusbar: Option<Rect>,
    pub commandbar: Option<Rect>,
    pub palette: Option<Rect>,
    pub search_panel: Option<Rect>,
    pub notifications: Option<Rect>,
}

impl UILayout {
    pub fn new() -> Self {
        Self {
            show_statusbar: true,
            show_commandbar: true,
            show_file_tree: false,
            show_markdown_preview: false,
            show_palette: false,
            show_search_panel: false,
            show_notifications: true,
            split_direction: SplitDirection::Horizontal,
            split_ratio: 0.55,
            filetree_width: 28,
            preview_width_ratio: 0.45,
            preview_as_markdown: false,
            focused_pane: FocusedPane::Editor,
        }
    }

    pub fn calculate_layout(&self, area: Rect) -> LayoutResult {
        let mut remaining = area;

        let notifications = if self.show_notifications {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(remaining);
            remaining = chunks[0];
            Some(chunks[1])
        } else {
            None
        };

        let commandbar = if self.show_commandbar {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(remaining);
            remaining = chunks[0];
            Some(chunks[1])
        } else {
            None
        };

        let statusbar = if self.show_statusbar {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(remaining);
            remaining = chunks[0];
            Some(chunks[1])
        } else {
            None
        };

        let palette = if self.show_palette {
            let palette_height = 20.min(remaining.height.saturating_sub(4));
            let top_margin = (remaining.height.saturating_sub(palette_height)) / 2;
            let palette_area = Rect::new(
                remaining.x + 4,
                remaining.y + top_margin,
                remaining.width.saturating_sub(8),
                palette_height,
            );
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(top_margin), Constraint::Min(0)])
                .split(remaining);
            remaining = chunks[1];
            Some(palette_area)
        } else {
            None
        };

        let search_result_area = if self.show_search_panel {
            let search_height = 12.min(remaining.height.saturating_sub(4));
            let top_margin = (remaining.height.saturating_sub(search_height)) / 2;
            let search_area = Rect::new(
                remaining.x + 2,
                remaining.y + top_margin,
                remaining.width.saturating_sub(4),
                search_height,
            );
            Some(search_area)
        } else {
            None
        };

        let (editor, filetree, markdown_preview) = if self.show_file_tree && self.show_markdown_preview {
            self.layout_triple_split(remaining)
        } else if self.show_file_tree {
            self.layout_double_split(remaining, true)
        } else if self.show_markdown_preview {
            self.layout_double_split(remaining, false)
        } else {
            (remaining, None, None)
        };

        let gutter = Rect::new(editor.x, editor.y, 0, editor.height);

        LayoutResult {
            editor,
            gutter,
            filetree,
            markdown_preview,
            statusbar,
            commandbar,
            palette,
            search_panel: search_result_area,
            notifications,
        }
    }

    fn layout_double_split(&self, area: Rect, filetree_first: bool) -> (Rect, Option<Rect>, Option<Rect>) {
        match self.split_direction {
            SplitDirection::Horizontal => {
                if filetree_first {
                    let filetree_width = self.filetree_width.min(area.width.saturating_sub(4));
                    let editor_width = area.width.saturating_sub(filetree_width);
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Length(filetree_width),
                            Constraint::Min(editor_width),
                        ])
                        .split(area);
                    (chunks[1], Some(chunks[0]), None)
                } else {
                    let preview_width = (area.width as f64 * self.preview_width_ratio) as u16;
                    let editor_width = area.width.saturating_sub(preview_width);
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Min(editor_width),
                            Constraint::Length(preview_width),
                        ])
                        .split(area);
                    (chunks[0], None, Some(chunks[1]))
                }
            }
            SplitDirection::Vertical => {
                if filetree_first {
                    let filetree_height = 12.min(area.height.saturating_sub(4));
                    let editor_height = area.height.saturating_sub(filetree_height);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(filetree_height),
                            Constraint::Min(editor_height),
                        ])
                        .split(area);
                    (chunks[1], Some(chunks[0]), None)
                } else {
                    let preview_height = (area.height as f64 * self.preview_width_ratio) as u16;
                    let editor_height = area.height.saturating_sub(preview_height);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Min(editor_height),
                            Constraint::Length(preview_height),
                        ])
                        .split(area);
                    (chunks[0], None, Some(chunks[1]))
                }
            }
        }
    }

    fn layout_triple_split(&self, area: Rect) -> (Rect, Option<Rect>, Option<Rect>) {
        match self.split_direction {
            SplitDirection::Horizontal => {
                let filetree_width = self.filetree_width.min(area.width.saturating_sub(6));
                let remaining_width = area.width.saturating_sub(filetree_width);
                let preview_width = ((remaining_width as f64) * self.preview_width_ratio) as u16;
                let editor_width = remaining_width.saturating_sub(preview_width);

                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(filetree_width),
                        Constraint::Min(editor_width.max(10)),
                        Constraint::Length(preview_width.max(10)),
                    ])
                    .split(area);
                (chunks[1], Some(chunks[0]), Some(chunks[2]))
            }
            SplitDirection::Vertical => {
                let filetree_height = 12.min(area.height.saturating_sub(6));
                let remaining_height = area.height.saturating_sub(filetree_height);
                let preview_height = ((remaining_height as f64) * self.preview_width_ratio) as u16;
                let editor_height = remaining_height.saturating_sub(preview_height);

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(filetree_height),
                        Constraint::Min(editor_height.max(3)),
                        Constraint::Length(preview_height.max(3)),
                    ])
                    .split(area);
                (chunks[1], Some(chunks[0]), Some(chunks[2]))
            }
        }
    }

    pub fn toggle_filetree(&mut self) {
        self.show_file_tree = !self.show_file_tree;
    }

    pub fn toggle_markdown_preview(&mut self) {
        self.show_markdown_preview = !self.show_markdown_preview;
    }

    pub fn toggle_side_by_side(&mut self) {
        self.show_markdown_preview = !self.show_markdown_preview;
    }

    pub fn toggle_palette(&mut self) {
        self.show_palette = !self.show_palette;
    }

    pub fn cycle_split_direction(&mut self) {
        self.split_direction = match self.split_direction {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        };
    }
}
