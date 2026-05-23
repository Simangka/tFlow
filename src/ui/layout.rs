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
    pub show_staging_panel: bool,
    pub focused_pane: FocusedPane,
    pub split_direction: SplitDirection,
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
    pub staging_panel: Option<Rect>,
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
			show_staging_panel: false,
			split_direction: SplitDirection::Horizontal,
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
            Some(Rect::new(
                remaining.x + 4,
                remaining.y + top_margin,
                remaining.width.saturating_sub(8),
                palette_height,
            ))
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

        let (editor, staging_panel) = if self.show_staging_panel {
            let staging_width = 36u16.min(editor.width.saturating_sub(10));
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Length(staging_width)])
                .split(editor);
            (chunks[0], Some(chunks[1]))
        } else {
            (editor, None)
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
            staging_panel,
        }
    }

    fn layout_double_split(&self, area: Rect, filetree_first: bool) -> (Rect, Option<Rect>, Option<Rect>) {
        let ft_w = 28u16;
        let preview_w = (area.width as f64 * 0.45) as u16;
        match self.split_direction {
            SplitDirection::Horizontal => {
                if filetree_first {
                    let fw = ft_w.min(area.width.saturating_sub(4));
                    let ew = area.width.saturating_sub(fw);
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(fw), Constraint::Min(ew)])
                        .split(area);
                    (chunks[1], Some(chunks[0]), None)
                } else {
                    let pw = preview_w.min(area.width.saturating_sub(4));
                    let ew = area.width.saturating_sub(pw);
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Min(ew), Constraint::Length(pw)])
                        .split(area);
                    (chunks[0], None, Some(chunks[1]))
                }
            }
            SplitDirection::Vertical => {
                if filetree_first {
                    let fh = 12.min(area.height.saturating_sub(4));
                    let eh = area.height.saturating_sub(fh);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(fh), Constraint::Min(eh)])
                        .split(area);
                    (chunks[1], Some(chunks[0]), None)
                } else {
                    let ph = preview_w.min(area.height.saturating_sub(4));
                    let eh = area.height.saturating_sub(ph);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(eh), Constraint::Length(ph)])
                        .split(area);
                    (chunks[0], None, Some(chunks[1]))
                }
            }
        }
    }

    fn layout_triple_split(&self, area: Rect) -> (Rect, Option<Rect>, Option<Rect>) {
        let ft_w = 28u16;
        match self.split_direction {
            SplitDirection::Horizontal => {
                let fw = ft_w.min(area.width.saturating_sub(6));
                let rw = area.width.saturating_sub(fw);
                let pw = ((rw as f64) * 0.45) as u16;
                let ew = rw.saturating_sub(pw);
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Length(fw),
                        Constraint::Min(ew.max(10)),
                        Constraint::Length(pw.max(10)),
                    ])
                    .split(area);
                (chunks[1], Some(chunks[0]), Some(chunks[2]))
            }
            SplitDirection::Vertical => {
                let fh = 12.min(area.height.saturating_sub(6));
                let rh = area.height.saturating_sub(fh);
                let ph = ((rh as f64) * 0.45) as u16;
                let eh = rh.saturating_sub(ph);
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(fh),
                        Constraint::Min(eh.max(3)),
                        Constraint::Length(ph.max(3)),
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
