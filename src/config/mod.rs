pub mod settings;

use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct EditorConfig {
    pub tab_size: usize,
    pub scrolloff: usize,
    pub word_wrap: bool,
    pub syntax_highlighting: bool,
    pub cursor_style: String,
    pub show_tabs: bool,
    pub show_whitespace: bool,
    pub auto_save: bool,
    pub auto_save_interval: u64,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            scrolloff: 3,
            word_wrap: true,
            syntax_highlighting: true,
            cursor_style: "block".to_string(),
            show_tabs: false,
            show_whitespace: false,
            auto_save: false,
            auto_save_interval: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LineNumbersConfig {
    pub show: bool,
    pub relative: bool,
}

impl Default for LineNumbersConfig {
    fn default() -> Self {
        Self {
            show: true,
            relative: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceConfig {
    pub root_path: Option<PathBuf>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self { root_path: None }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeymapConfig {
    pub custom: Option<String>,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self { custom: None }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarkdownConfig {
    pub preview: bool,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self { preview: true }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerminalConfig {
    pub position: String,
    pub height: u16,
    pub width: u16,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            position: "right".to_string(),
            height: 12,
            width: 100,
        }
    }
}

/// Persistent configuration loaded from the TOML config file.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub theme: String,
    pub line_numbers: LineNumbersConfig,
    pub editor: EditorConfig,
    pub workspace: WorkspaceConfig,
    pub keymap: KeymapConfig,
    pub markdown: MarkdownConfig,
    pub terminal: TerminalConfig,
    pub clipboard_provider: String,
    pub files: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "default_dark".to_string(),
            line_numbers: LineNumbersConfig::default(),
            editor: EditorConfig::default(),
            workspace: WorkspaceConfig::default(),
            keymap: KeymapConfig::default(),
            markdown: MarkdownConfig::default(),
            terminal: TerminalConfig::default(),
            clipboard_provider: "system".to_string(),
            files: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = settings::default_config_path();
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str::<Config>(&content) {
                return config;
            }
        }
        Config::default()
    }

    pub fn merge_cli_overrides(
        &mut self,
        theme: Option<&str>,
        no_line_numbers: bool,
        _verbose: bool,
        _log_file: Option<&std::path::Path>,
        _command: Option<&str>,
        workspace: Option<&std::path::Path>,
        _position: Option<&str>,
        _readonly: bool,
        files: &[String],
    ) {
        if let Some(t) = theme {
            self.theme = t.to_string();
        }
        if no_line_numbers {
            self.line_numbers.show = false;
        }
        if let Some(ws) = workspace {
            self.workspace.root_path = Some(ws.to_path_buf());
        }
        self.files = files.to_vec();
    }
}

/// CLI-specific options that are NOT persisted in the config file.
#[derive(Debug, Clone)]
pub struct CliOptions {
    pub readonly: bool,
    pub verbose: bool,
    pub log_file: Option<PathBuf>,
    pub files: Vec<String>,
    pub command: Option<String>,
    pub position: Option<String>,
    pub no_line_numbers: bool,
}
