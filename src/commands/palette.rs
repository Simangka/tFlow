use crate::commands::actions::{Action, ActionCategory};
use crate::commands::registry::CommandRegistry;
use crate::commands::keymap::KeyMap;
use std::time::{Duration, Instant};

const MAX_QUERY_LEN: usize = 256;
const MAX_ITEMS: usize = 10_000;
const QUERY_DEBOUNCE_MS: u64 = 16;

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
    pub labels_lower: Vec<String>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub mode: PaletteMode,
    pub last_query_time: Option<Instant>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            cursor: 0,
            items: Vec::new(),
            labels_lower: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            mode: PaletteMode::Commands,
            last_query_time: None,
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
        if self.query.chars().count() >= MAX_QUERY_LEN {
            return;
        }
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
        let now = Instant::now();
        if let Some(last) = self.last_query_time {
            if now.duration_since(last) < Duration::from_millis(QUERY_DEBOUNCE_MS) {
                return;
            }
        }
        self.last_query_time = Some(now);
        let truncated: String = query.chars().take(MAX_QUERY_LEN).collect();
        let truncated_len = truncated.chars().count();
        self.query = truncated;
        self.cursor = truncated_len;
        self.filter_items();
        self.selected = 0;
    }

    pub fn filter_items(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.items.len()).collect();
            return;
        }
        let query_lower: Vec<char> = self.query.chars().flat_map(|c| c.to_lowercase()).collect();
        let mut scored: Vec<(usize, f64)> = self.items.iter().enumerate().map(|(i, _item)| {
            let label_lower = self.labels_lower.get(i).map(|s| s.as_str()).unwrap_or("");
            let score = Self::fuzzy_score_cached(&query_lower, label_lower);
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
        self.labels_lower.clear();
        for cmd in registry.all_commands() {
            let action = cmd.action.clone().unwrap_or(Action::NoOp);
            let cat = action.category();
            let keys = keymap.describe_binding(&action);
            self.labels_lower.push(cmd.name.to_lowercase());
            self.items.push(PaletteItem {
                label: cmd.name.clone(),
                description: cmd.description.clone(),
                action: PaletteAction::Command(cmd.name.clone()),
                category: cat,
                keys,
                score: 0.0,
            });
            if self.items.len() >= MAX_ITEMS {
                break;
            }
        }
        self.filter_items();
    }

    pub fn set_files(&mut self, files: Vec<String>) {
        self.items.clear();
        self.labels_lower.clear();
        for file in files {
            if self.items.len() >= MAX_ITEMS {
                break;
            }
            let path = std::path::Path::new(&file);
            let name = path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| file.clone());
            self.labels_lower.push(name.to_lowercase());
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
        self.labels_lower.clear();
        for (name, kind) in symbols {
            if self.items.len() >= MAX_ITEMS {
                break;
            }
            let label = name.clone();
            self.labels_lower.push(label.to_lowercase());
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
        self.labels_lower.clear();
        for (text, level) in headings {
            if self.items.len() >= MAX_ITEMS {
                break;
            }
            let prefix = "#".repeat(level);
            let label = format!("{} {}", prefix, text);
            self.labels_lower.push(label.to_lowercase());
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
        let text_lower: String = text.chars().flat_map(|c| c.to_lowercase()).collect();
        Self::fuzzy_score_cached(&query_lower, &text_lower)
    }

    pub fn fuzzy_score_cached(query_lower: &[char], text_lower_str: &str) -> f64 {
        if query_lower.is_empty() {
            return 1.0;
        }
        if text_lower_str.is_empty() {
            return 0.0;
        }
        let text_lower: Vec<char> = text_lower_str.chars().collect();
        if query_lower.len() > text_lower.len() {
            return 0.0;
        }
        let mut score = 0.0;
        let mut query_idx = 0;
        let mut prev_match = false;
        let mut first_match = true;
        let mut match_count = 0;
        let mut first_match_idx: Option<usize> = None;
        let mut last_match_idx: Option<usize> = None;
        for (i, &tc) in text_lower.iter().enumerate() {
            if query_idx < query_lower.len() && tc == query_lower[query_idx] {
                if first_match_idx.is_none() {
                    first_match_idx = Some(i);
                }
                last_match_idx = Some(i);
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
                if let Some(last) = last_match_idx {
                    if i == last {
                        match_score += 3.0;
                    }
                }
                score += match_score;
                prev_match = true;
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
            let first_idx = first_match_idx.unwrap_or(0);
            let last_idx = last_match_idx.unwrap_or(0);
            let span = last_idx.saturating_sub(first_idx) as f64;
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
        let prefix_bonus = if text_lower.starts_with(query_lower) {
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
