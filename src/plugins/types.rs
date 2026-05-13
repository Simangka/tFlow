use std::path::PathBuf;

pub type PluginId = u64;

#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub entry_point: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PluginCapabilities {
    pub can_handle_key_events: bool,
    pub can_intercept_buffer: bool,
    pub can_render_overlay: bool,
    pub can_register_commands: bool,
}

#[derive(Debug, Clone)]
pub enum PluginEvent {
    Initialized,
    KeyPressed { key: String, modifiers: Vec<String> },
    BufferChanged { buffer_id: usize, changes: Vec<BufferChange> },
    Saved { buffer_id: usize, path: PathBuf },
    ModeChanged { old_mode: String, new_mode: String },
    RenderFrame { delta_time: f64 },
    ShuttingDown,
    Custom(String, serde_json::Value),
}

#[derive(Debug, Clone)]
pub struct BufferChange {
    pub line: usize,
    pub column: usize,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone)]
pub struct PluginHost {
    pub metadata: PluginMetadata,
    pub capabilities: PluginCapabilities,
    pub state: serde_json::Value,
    pub config: serde_json::Value,
}
