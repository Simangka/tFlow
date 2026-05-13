pub fn validate_theme_name(name: &str) -> bool {
    matches!(name, "default_dark" | "retro_green" | "amber" | "synthwave" | "tokyo_night")
}

pub fn default_config_path() -> std::path::PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
    base.join("tflow").join("config.toml")
}
