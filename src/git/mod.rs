pub mod manager;
pub mod blame;
pub mod status;
pub mod operations;
pub mod staging_panel;
pub mod branch_view;
pub mod graph_renderer;

pub const GIT_BLOCKING_WARNING: &str = "git ops are blocking; call from spawn_blocking";

pub use manager::GitManager;
pub use blame::BlameInfo;
pub use status::RepoStatus;
pub use staging_panel::{StagingPanel, StagingEntry};
pub use branch_view::BranchViewPanel;
pub use graph_renderer::GraphRenderer;
