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
            word_wrap: false,
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

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub theme: String,
    pub line_numbers: LineNumbersConfig,
    pub editor: EditorConfig,
    pub workspace: WorkspaceConfig,
    pub keymap: KeymapConfig,
    pub markdown: MarkdownConfig,
    pub terminal: TerminalConfig,
    pub readonly: bool,
    pub verbose: bool,
    pub log_file: Option<PathBuf>,
    pub files: Vec<String>,
    pub command: Option<String>,
    pub position: Option<String>,
    pub clipboard_provider: String,
    pub no_line_numbers: bool,
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
            readonly: false,
            verbose: false,
            log_file: None,
            files: Vec::new(),
            command: None,
            position: None,
            clipboard_provider: "system".to_string(),
            no_line_numbers: false,
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
        verbose: bool,
        log_file: Option<&std::path::Path>,
        command: Option<&str>,
        workspace: Option<&std::path::Path>,
        position: Option<&str>,
        readonly: bool,
        files: &[String],
    ) {
        if let Some(t) = theme {
            self.theme = t.to_string();
        }
        if no_line_numbers {
            self.line_numbers.show = false;
        }
        self.verbose |= verbose;
        if let Some(lf) = log_file {
            self.log_file = Some(lf.to_path_buf());
        }
        if let Some(cmd) = command {
            self.command = Some(cmd.to_string());
        }
        if let Some(ws) = workspace {
            self.workspace.root_path = Some(ws.to_path_buf());
        }
        if let Some(pos) = position {
            self.position = Some(pos.to_string());
        }
        if readonly {
            self.readonly = true;
        }
        if !files.is_empty() {
            self.files = files.to_vec();
        }
    }
}
