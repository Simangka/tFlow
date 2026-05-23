use std::path::PathBuf;
use crate::git::graph_renderer::GraphRenderer;

#[derive(Debug, Clone)]
pub struct GraphLine {
    pub text: String,
    pub branch: Option<String>,
    pub is_head: bool,
}

#[derive(Debug, Clone)]
pub struct BranchViewPanel {
    pub visible: bool,
    pub data: Vec<GraphLine>,
    pub selected: usize,
    pub repo_path: Option<PathBuf>,
}

impl BranchViewPanel {
    pub fn new() -> Self {
        Self { visible: false, data: Vec::new(), selected: 0, repo_path: None }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.selected = 0;
        }
    }

    pub fn refresh(&mut self, repo_path: PathBuf) {
        self.repo_path = Some(repo_path.clone());
        self.data.clear();

        let repo = match git2::Repository::open(&repo_path) {
            Ok(r) => r,
            Err(_) => return,
        };

        let renderer = GraphRenderer::new(&repo);

        for row in &renderer.rows {
            self.data.push(GraphLine {
                text: row.text.clone(),
                branch: row.branch.clone(),
                is_head: row.is_head,
            });
        }

        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.data.is_empty() {
            self.selected = (self.selected + 1).min(self.data.len().saturating_sub(1));
        }
    }

    pub fn select_prev(&mut self) {
        if !self.data.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn selected_branch(&self) -> Option<String> {
        self.data.get(self.selected).and_then(|g| g.branch.clone())
    }
}
