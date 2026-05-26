pub mod cache;
pub mod client;
pub mod config;
pub mod handlers;
pub mod manager;
pub mod rpc;
pub mod sync;
pub mod types;

pub use cache::LspCache;
pub use client::LanguageClient;
pub use config::LanguageServerConfig;
pub use manager::{LspManager, run_lsp_manager};
pub use types::*;

use tokio::sync::mpsc;

pub fn create_lsp_channels() -> (
    mpsc::UnboundedSender<LspCommand>,
    mpsc::UnboundedReceiver<LspEvent>,
    mpsc::UnboundedReceiver<LspCommand>,
    mpsc::UnboundedSender<LspEvent>,
) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    (cmd_tx, event_rx, cmd_rx, event_tx)
}
