pub mod types;

pub use types::*;

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

pub struct PluginManager {
    pub plugins: HashMap<String, Plugin>,
    pub plugin_dir: std::path::PathBuf,
}

pub struct Plugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub hooks: Vec<PluginHook>,
    pub enabled: bool,
    pub config: serde_json::Value,
}

pub enum PluginHook {
    OnInit,
    OnKeyPress,
    OnBufferChange,
    OnSave,
    OnModeChange,
    OnRender,
    OnExit,
    Custom(String),
}

fn name_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[a-zA-Z0-9_.-]{1,64}$").expect("valid name regex")
    })
}

fn version_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\d+\.\d+\.\d+$").expect("valid version regex")
    })
}

const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

impl PluginManager {
    pub fn new(plugin_dir: std::path::PathBuf) -> Self {
        PluginManager {
            plugins: HashMap::new(),
            plugin_dir,
        }
    }

    pub fn discover_plugins(&mut self) -> Result<(), anyhow::Error> {
        if !self.plugin_dir.exists() {
            std::fs::create_dir_all(&self.plugin_dir)?;
            return Ok(());
        }

        let read_dir = std::fs::read_dir(&self.plugin_dir)?;
        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("plugin.toml");
                if manifest_path.exists() {
                    self.load_plugin(&path)?;
                }
            } else if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                self.load_plugin(&path)?;
            }
        }

        Ok(())
    }

    pub fn enable_plugin(&mut self, name: &str) -> Result<(), anyhow::Error> {
        if !self.plugins.contains_key(name) {
            return Err(anyhow::anyhow!("Plugin '{}' not found", name));
        }
        if let Some(plugin) = self.plugins.get_mut(name) {
            plugin.enabled = true;
        }
        Ok(())
    }

    pub fn disable_plugin(&mut self, name: &str) {
        if let Some(plugin) = self.plugins.get_mut(name) {
            plugin.enabled = false;
        }
    }

    pub fn load_plugin(&mut self, path: &Path) -> Result<(), anyhow::Error> {
        if path.is_dir() {
            let manifest_path = path.join("plugin.toml");

            let file_type = std::fs::symlink_metadata(&manifest_path)?.file_type();
            if file_type.is_symlink() {
                anyhow::bail!("symlink manifests rejected");
            }
            if !file_type.is_file() {
                anyhow::bail!("manifest is not a file");
            }

            let meta = std::fs::metadata(&manifest_path)?;
            if meta.len() > MAX_MANIFEST_BYTES {
                anyhow::bail!("manifest too large");
            }

            let content = std::fs::read_to_string(&manifest_path)?;
            let manifest: PluginManifest = toml::from_str(&content)?;
            manifest.validate()?;

            if manifest.host_version != env!("CARGO_PKG_VERSION") {
                anyhow::bail!(
                    "plugin host_version '{}' does not match host version '{}'",
                    manifest.host_version,
                    env!("CARGO_PKG_VERSION")
                );
            }

            let entry_path = path.join(&manifest.entry);
            let entry_meta = std::fs::metadata(&entry_path)?;
            if !entry_meta.is_file() {
                anyhow::bail!("plugin entry is not a file");
            }
            let mut entry_file = std::fs::File::open(&entry_path)?;
            let mut buf = [0u8; 4];
            entry_file.read_exact(&mut buf)?;
            if buf != WASM_MAGIC {
                anyhow::bail!("plugin entry missing wasm magic bytes");
            }

            let plugin = Plugin {
                name: manifest.name.clone(),
                version: manifest.version,
                description: manifest.description.unwrap_or_default(),
                author: manifest.author.unwrap_or_default(),
                hooks: Vec::new(),
                enabled: false,
                config: serde_json::Value::Object(serde_json::Map::new()),
            };
            self.plugins.insert(manifest.name, plugin);
        } else {
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let plugin = Plugin {
                name: file_stem.clone(),
                version: "0.1.0".to_string(),
                description: format!("WASM plugin: {}", file_stem),
                author: "Unknown".to_string(),
                hooks: Vec::new(),
                enabled: false,
                config: serde_json::Value::Object(serde_json::Map::new()),
            };
            self.plugins.insert(file_stem, plugin);
        }

        Ok(())
    }

    pub fn get_plugin(&self, name: &str) -> Option<&Plugin> {
        self.plugins.get(name)
    }

    pub fn list_plugins(&self) -> Vec<&Plugin> {
        self.plugins.values().collect()
    }

    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugins.get(name).map(|p| p.enabled).unwrap_or(false)
    }

    pub fn enabled_plugins(&self) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|(_, p)| p.enabled)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

#[derive(serde::Deserialize)]
struct PluginManifest {
    name: String,
    version: String,
    description: Option<String>,
    author: Option<String>,
    host_version: String,
    entry: PathBuf,
}

impl PluginManifest {
    fn validate(&self) -> Result<(), anyhow::Error> {
        if !name_regex().is_match(&self.name) {
            anyhow::bail!("invalid plugin name: must match ^[a-zA-Z0-9_.-]{{1,64}}$");
        }
        if !version_regex().is_match(&self.version) {
            anyhow::bail!("invalid plugin version: must be semver-ish (e.g. 0.1.0)");
        }
        if let Some(desc) = &self.description {
            if desc.len() > 1024 {
                anyhow::bail!("plugin description too long (max 1024 bytes)");
            }
            if desc.chars().any(|c| c.is_control()) {
                anyhow::bail!("plugin description contains control characters");
            }
        }
        if let Some(author) = &self.author {
            if author.len() > 256 {
                anyhow::bail!("plugin author too long (max 256 bytes)");
            }
            if author.chars().any(|c| c.is_control()) {
                anyhow::bail!("plugin author contains control characters");
            }
        }
        Ok(())
    }
}
