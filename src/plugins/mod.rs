pub mod types;

pub use types::*;

use std::collections::HashMap;
use std::path::Path;

pub struct PluginManager {
    pub plugins: HashMap<String, Plugin>,
    pub enabled: Vec<String>,
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

impl PluginManager {
    pub fn new(plugin_dir: std::path::PathBuf) -> Self {
        PluginManager {
            plugins: HashMap::new(),
            enabled: Vec::new(),
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
        if !self.enabled.contains(&name.to_string()) {
            self.enabled.push(name.to_string());
        }
        Ok(())
    }

    pub fn disable_plugin(&mut self, name: &str) {
        if let Some(plugin) = self.plugins.get_mut(name) {
            plugin.enabled = false;
        }
        self.enabled.retain(|e| e != name);
    }

    pub fn load_plugin(&mut self, path: &Path) -> Result<(), anyhow::Error> {
        if path.is_dir() {
            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                return Err(anyhow::anyhow!("No plugin.toml found in {:?}", path));
            }
            let content = std::fs::read_to_string(&manifest_path)?;
            let manifest: PluginManifest = toml::from_str(&content)?;
            let plugin = Plugin {
                name: manifest.name,
                version: manifest.version,
                description: manifest.description.unwrap_or_default(),
                author: manifest.author.unwrap_or_default(),
                hooks: Vec::new(),
                enabled: false,
                config: serde_json::Value::Object(serde_json::Map::new()),
            };
            let name = plugin.name.clone();
            self.plugins.insert(name, plugin);
        } else {
            let file_stem = path.file_stem()
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
}

#[derive(serde::Deserialize)]
struct PluginManifest {
    name: String,
    version: String,
    description: Option<String>,
    author: Option<String>,
}
