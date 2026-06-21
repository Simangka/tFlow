use crate::commands::actions::{Action, ActionCategory};
use crate::commands::registry::CommandRegistry;
use crate::commands::keymap::KeyMap;

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub label: String,
    pub description: String,
    pub action: PaletteAction,
    pub category: ActionCategory,
    pub keys: Vec<String>,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub enum PaletteAction {
    Command(String),
    Action(Action),
    File(String),
    Symbol(String),
    Heading(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteMode {
    Commands,
    Files,
    Symbols,
    Headings,
    Search,
    Grep,
}

#[derive(Debug, Clone)]
pub struct CommandPalette {
    pub visible: bool,
    pub query: String,
    pub cursor: usize,
    pub items: Vec<PaletteItem>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub mode: PaletteMode,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            cursor: 0,
            items: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            mode: PaletteMode::Commands,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.query.clear();
            self.cursor = 0;
            self.selected = 0;
            self.filtered.clear();
        }
    }

    pub fn show(&mut self, mode: PaletteMode) {
        self.visible = true;
        self.mode = mode;
        self.query.clear();
        self.cursor = 0;
        self.selected = 0;
        self.filter_items();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.query.clear();
        self.cursor = 0;
        self.selected = 0;
        self.filtered.clear();
    }

    pub fn push_char(&mut self, c: char) {
        self.query.insert(self.cursor, c);
        self.cursor += 1;
        self.filter_items();
        self.selected = 0;
    }

    pub fn pop_char(&mut self) {
        if self.cursor > 0 && !self.query.is_empty() {
            self.cursor -= 1;
            self.query.remove(self.cursor);
            self.filter_items();
            self.selected = 0;
        }
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.cursor = query.len();
        self.filter_items();
        self.selected = 0;
    }

    pub fn filter_items(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.items.len()).collect();
            return;
        }
        let mut scored: Vec<(usize, f64)> = self.items.iter().enumerate().map(|(i, item)| {
            let score = Self::fuzzy_score(&self.query, &item.label);
            (i, score)
        }).filter(|(_, score)| *score > 0.0).collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.filtered.get(self.selected).and_then(|&idx| self.items.get(idx))
    }

    pub fn set_commands(&mut self, registry: &CommandRegistry, keymap: &KeyMap) {
        self.items.clear();
        for cmd in registry.all_commands() {
            let action = cmd.action.clone().unwrap_or(Action::Noop);
            let cat = action.category();
            let keys = keymap.describe_binding(&action);
            self.items.push(PaletteItem {
                label: cmd.name.clone(),
                description: cmd.description.clone(),
                action: PaletteAction::Command(cmd.name.clone()),
                category: cat,
                keys,
                score: 0.0,
            });
        }
        self.filter_items();
    }

    pub fn set_files(&mut self, files: Vec<String>) {
        self.items.clear();
        for file in files {
            let path = std::path::Path::new(&file);
            let name = path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| file.clone());
            self.items.push(PaletteItem {
                label: name,
                description: file.clone(),
                action: PaletteAction::File(file),
                category: ActionCategory::File,
                keys: Vec::new(),
                score: 0.0,
            });
        }
        self.filter_items();
    }

    pub fn set_symbols(&mut self, symbols: Vec<(String, String)>) {
        self.items.clear();
        for (name, kind) in symbols {
            let label = name.clone();
            self.items.push(PaletteItem {
                label,
                description: kind,
                action: PaletteAction::Symbol(name),
                category: ActionCategory::Navigation,
                keys: Vec::new(),
                score: 0.0,
            });
        }
        self.filter_items();
    }

    pub fn set_headings(&mut self, headings: Vec<(String, usize)>) {
        self.items.clear();
        for (text, level) in headings {
            let prefix = "#".repeat(level);
            let label = format!("{} {}", prefix, text);
            self.items.push(PaletteItem {
                label: label.clone(),
                description: format!("Heading level {}", level),
                action: PaletteAction::Heading(text),
                category: ActionCategory::Navigation,
                keys: Vec::new(),
                score: 0.0,
            });
        }
        self.filter_items();
    }

    pub fn fuzzy_score(query: &str, text: &str) -> f64 {
        if query.is_empty() {
            return 1.0;
        }
        if text.is_empty() {
            return 0.0;
        }
        let query_lower: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
        let text_lower: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
        if query_lower.len() > text_lower.len() {
            return 0.0;
        }
        let mut score = 0.0;
        let mut query_idx = 0;
        let mut prev_match = false;
        let mut first_match = true;
        let mut match_count = 0;
        let mut last_match_end = 0;
        for (i, &tc) in text_lower.iter().enumerate() {
            if query_idx < query_lower.len() && tc == query_lower[query_idx] {
                query_idx += 1;
                match_count += 1;
                let mut match_score = 10.0;
                if first_match {
                    if i == 0 {
                        match_score += 10.0;
                    }
                    first_match = false;
                }
                if prev_match {
                    match_score += 8.0;
                }
                if i > 0 {
                    let prev_char = text_lower[i - 1];
                    if prev_char == '_' || prev_char == '-' || prev_char == '.' || prev_char == '/' || prev_char == '\\' {
                        match_score += 15.0;
                    }
                    if prev_char == ' ' || prev_char == '\t' {
                        match_score += 12.0;
                    }
                    if prev_char.is_ascii_uppercase() && text_lower[i] != prev_char {
                        match_score += 10.0;
                    }
                    if prev_char.is_ascii_lowercase() && text_lower[i] != prev_char && text_lower[i].is_ascii_uppercase() {
                        match_score += 15.0;
                    }
                    if prev_char.is_ascii_digit() && !text_lower[i].is_ascii_digit() {
                        match_score += 8.0;
                    }
                }
                if i == last_match_end {
                    match_score += 3.0;
                }
                score += match_score;
                prev_match = true;
                last_match_end = i + 1;
            } else {
                prev_match = false;
            }
        }
        if query_idx < query_lower.len() {
            return 0.0;
        }
        let text_len = text_lower.len() as f64;
        let query_len = query_lower.len() as f64;
        let proximity_bonus = if match_count > 1 {
            let span = (last_match_end - (text_lower.iter().position(|&_c| {
                let mut qi = 0;
                for &tc2 in text_lower.iter() {
                    if qi < query_lower.len() && tc2 == query_lower[qi] {
                        qi += 1;
                        if qi == query_lower.len() {
                            return false;
                        }
                    }
                }
                false
            }).unwrap_or(0))) as f64;
            if span > 0.0 { (query_len / span) * 10.0 } else { 0.0 }
        } else {
            0.0
        };
        score += proximity_bonus;
        let exact_match_bonus = if query_lower.iter().zip(text_lower.iter()).take(query_lower.len()).all(|(q, t)| q == t) {
            if query_lower.len() == text_lower.len() {
                50.0
            } else {
                20.0
            }
        } else {
            0.0
        };
        score += exact_match_bonus;
        let prefix_bonus = if text_lower.starts_with(&query_lower) {
            15.0
        } else {
            0.0
        };
        score += prefix_bonus;
        let match_ratio = match_count as f64 / query_len;
        score *= match_ratio;
        if score <= 0.0 {
            return 0.0;
        }
        let normalized = score / (text_len * 10.0 + 50.0);
        normalized.min(1.0).max(0.0)
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}
