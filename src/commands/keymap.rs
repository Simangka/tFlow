use crate::commands::actions::Action;
use crate::core::EditMode;
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub key: KeyEvent,
    pub action: Action,
    pub mode: Option<EditMode>,
    pub description: String,
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn ctrl_shift(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c.to_ascii_uppercase()), KeyModifiers::CONTROL | KeyModifiers::SHIFT)
}

fn char_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn shift_char(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
}

fn alt_char(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

#[derive(Debug, Clone)]
pub struct KeyMap {
    pub bindings: HashMap<(KeyEvent, Option<EditMode>), Action>,
    pub sequence_bindings: HashMap<(KeyCode, KeyCode, Option<EditMode>), Action>,
    pub default_bindings: HashMap<(KeyEvent, Option<EditMode>), Action>,
    pub default_sequence_bindings: HashMap<(KeyCode, KeyCode, Option<EditMode>), Action>,
    pub leader_keys: Vec<KeyCode>,
    pub pending_keys: Vec<KeyEvent>,
    pub timeout_duration: std::time::Duration,
    pub last_key_time: Option<std::time::Instant>,
}

impl KeyMap {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            sequence_bindings: HashMap::new(),
            default_bindings: HashMap::new(),
            default_sequence_bindings: HashMap::new(),
            leader_keys: Vec::new(),
            pending_keys: Vec::new(),
            timeout_duration: std::time::Duration::from_millis(800),
            last_key_time: None,
        }
    }

    pub fn new_with_defaults() -> Self {
        let mut km = Self::new();

        km.leader_keys = vec![
            KeyCode::Char('g'),
            KeyCode::Char('d'),
            KeyCode::Char('y'),
            KeyCode::Char('c'),
            KeyCode::Char('z'),
            KeyCode::Char('w'),
        ];

        let normal = Some(EditMode::Normal);
        let insert = Some(EditMode::Insert);
        let visual = Some(EditMode::Visual);
        let vline = Some(EditMode::VisualLine);
        let command = Some(EditMode::Command);
        let search = Some(EditMode::Search);
        let none = None;

        let add = |km: &mut KeyMap,
                   key: KeyEvent,
                   action: Action,
                   mode: Option<EditMode>| {
            km.bindings.insert((key, mode), action.clone());
            km.default_bindings.insert((key, mode), action);
        };

        let add_seq = |km: &mut KeyMap,
                        first: KeyCode,
                        second: KeyCode,
                        action: Action,
                        mode: Option<EditMode>| {
            km.sequence_bindings.insert((first, second, mode), action.clone());
            km.default_sequence_bindings.insert((first, second, mode), action);
        };

        add(&mut km, char_key('h'), Action::MoveLeft, normal);
        add(&mut km, char_key('j'), Action::MoveDown, normal);
        add(&mut km, char_key('k'), Action::MoveUp, normal);
        add(&mut km, char_key('l'), Action::MoveRight, normal);
        add(&mut km, char_key('w'), Action::WordForward, normal);
        add(&mut km, char_key('b'), Action::WordBackward, normal);
        add(&mut km, char_key('e'), Action::WordForward, normal);
        add(&mut km, char_key('0'), Action::StartOfLine, normal);
        add(&mut km, char_key('^'), Action::StartOfLine, normal);
        add(&mut km, shift_char('$'), Action::EndOfLine, normal);
        add(&mut km, shift_char('G'), Action::EndOfFile, normal);
        add(&mut km, char_key('%'), Action::MoveToMatchingBrace, normal);
        add(&mut km, ctrl('u'), Action::HalfPageUp, normal);
        add(&mut km, ctrl('d'), Action::HalfPageDown, normal);
        add(&mut km, ctrl('b'), Action::PageUp, normal);
        add(&mut km, ctrl('f'), Action::PageDown, normal);

        add(&mut km, char_key('i'), Action::SwitchToInsertMode, normal);
        add(&mut km, char_key('a'), Action::SwitchToInsertMode, normal);
        add(&mut km, shift_char('I'), Action::SwitchToInsertMode, normal);
        add(&mut km, shift_char('A'), Action::SwitchToInsertMode, normal);
        add(&mut km, char_key('o'), Action::InsertNewline, normal);
        add(&mut km, shift_char('O'), Action::InsertNewline, normal);
        add(&mut km, char_key('v'), Action::SwitchToVisualMode, normal);
        add(&mut km, shift_char('V'), Action::SwitchToVisualLineMode, normal);
        add(&mut km, key(KeyCode::Esc, KeyModifiers::NONE), Action::SwitchToNormalMode, normal);
        add(&mut km, key(KeyCode::Esc, KeyModifiers::NONE), Action::SwitchToNormalMode, insert);
        add(&mut km, key(KeyCode::Esc, KeyModifiers::NONE), Action::SwitchToNormalMode, visual);
        add(&mut km, key(KeyCode::Esc, KeyModifiers::NONE), Action::SwitchToNormalMode, vline);
        add(&mut km, key(KeyCode::Esc, KeyModifiers::NONE), Action::SwitchToNormalMode, command);
        add(&mut km, key(KeyCode::Esc, KeyModifiers::NONE), Action::SwitchToNormalMode, search);
        add(&mut km, ctrl('c'), Action::SwitchToNormalMode, insert);
        add(&mut km, ctrl('c'), Action::SwitchToNormalMode, visual);
        add(&mut km, ctrl('['), Action::SwitchToNormalMode, insert);
        add(&mut km, ctrl('['), Action::SwitchToNormalMode, normal);

        add(&mut km, char_key('u'), Action::Undo, normal);
        add(&mut km, char_key('u'), Action::Undo, insert);
        add(&mut km, ctrl('r'), Action::Redo, normal);
        add(&mut km, ctrl('r'), Action::Redo, insert);

        add(&mut km, char_key('x'), Action::DeleteForward, normal);
        add(&mut km, char_key('X'), Action::DeleteBackward, normal);

        add_seq(&mut km, KeyCode::Char('d'), KeyCode::Char('d'), Action::DeleteLine, normal);
        add_seq(&mut km, KeyCode::Char('y'), KeyCode::Char('y'), Action::CopyLine, normal);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('g'), Action::StartOfFile, normal);

        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('v'), Action::SplitVertical, normal);
        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('s'), Action::SplitHorizontal, normal);
        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('q'), Action::ClosePane, normal);
        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('w'), Action::NextSplit, normal);
        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('h'), Action::FocusPaneLeft, normal);
        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('j'), Action::FocusPaneDown, normal);
        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('k'), Action::FocusPaneUp, normal);
        add_seq(&mut km, KeyCode::Char('w'), KeyCode::Char('l'), Action::FocusPaneRight, normal);

        add(&mut km, shift_char('D'), Action::DeleteToEndOfLine, normal);
        add(&mut km, shift_char('J'), Action::JoinLines, normal);
        add(&mut km, char_key('p'), Action::Paste, normal);
        add(&mut km, shift_char('P'), Action::Paste, normal);
        add(&mut km, key(KeyCode::Char('>'), KeyModifiers::NONE), Action::Indent, normal);
        add(&mut km, key(KeyCode::Char('>'), KeyModifiers::SHIFT), Action::Indent, normal);
        add(&mut km, key(KeyCode::Char('<'), KeyModifiers::NONE), Action::Unindent, normal);
        add(&mut km, key(KeyCode::Char('<'), KeyModifiers::SHIFT), Action::Unindent, normal);

        add(&mut km, key(KeyCode::Enter, KeyModifiers::NONE), Action::InsertNewline, insert);
        add(&mut km, key(KeyCode::Tab, KeyModifiers::NONE), Action::InsertTab, insert);
        add(&mut km, key(KeyCode::Backspace, KeyModifiers::NONE), Action::DeleteBackward, insert);
        add(&mut km, key(KeyCode::Delete, KeyModifiers::NONE), Action::DeleteForward, insert);
        add(&mut km, key(KeyCode::Backspace, KeyModifiers::NONE), Action::DeleteBackward, normal);
        add(&mut km, key(KeyCode::Delete, KeyModifiers::NONE), Action::DeleteForward, normal);
        add(&mut km, key(KeyCode::Left, KeyModifiers::NONE), Action::MoveLeft, insert);
        add(&mut km, key(KeyCode::Right, KeyModifiers::NONE), Action::MoveRight, insert);
        add(&mut km, key(KeyCode::Up, KeyModifiers::NONE), Action::MoveUp, insert);
        add(&mut km, key(KeyCode::Down, KeyModifiers::NONE), Action::MoveDown, insert);
        add(&mut km, key(KeyCode::Home, KeyModifiers::NONE), Action::StartOfLine, insert);
        add(&mut km, key(KeyCode::End, KeyModifiers::NONE), Action::EndOfLine, insert);
        add(&mut km, key(KeyCode::PageUp, KeyModifiers::NONE), Action::PageUp, insert);
        add(&mut km, key(KeyCode::PageDown, KeyModifiers::NONE), Action::PageDown, insert);

        add(&mut km, key(KeyCode::Left, KeyModifiers::NONE), Action::MoveLeft, normal);
        add(&mut km, key(KeyCode::Right, KeyModifiers::NONE), Action::MoveRight, normal);
        add(&mut km, key(KeyCode::Up, KeyModifiers::NONE), Action::MoveUp, normal);
        add(&mut km, key(KeyCode::Down, KeyModifiers::NONE), Action::MoveDown, normal);
        add(&mut km, key(KeyCode::Home, KeyModifiers::NONE), Action::StartOfLine, normal);
        add(&mut km, key(KeyCode::End, KeyModifiers::NONE), Action::EndOfLine, normal);
        add(&mut km, key(KeyCode::PageUp, KeyModifiers::NONE), Action::PageUp, normal);
        add(&mut km, key(KeyCode::PageDown, KeyModifiers::NONE), Action::PageDown, normal);

        add(&mut km, shift_char(':'), Action::SwitchToCommandMode, normal);
        add(&mut km, key(KeyCode::Enter, KeyModifiers::NONE), Action::Noop, command);
        add(&mut km, key(KeyCode::Backspace, KeyModifiers::NONE), Action::Noop, command);

        add(&mut km, char_key('/'), Action::Find, normal);
        add(&mut km, char_key('?'), Action::Find, normal);
        add(&mut km, char_key('n'), Action::FindNext, normal);
        add(&mut km, shift_char('N'), Action::FindPrevious, normal);

        add(&mut km, key(KeyCode::Enter, KeyModifiers::NONE), Action::Noop, search);
        add(&mut km, key(KeyCode::Backspace, KeyModifiers::NONE), Action::Noop, search);

        add(&mut km, ctrl('s'), Action::SaveFile, none);
        add(&mut km, ctrl('q'), Action::Quit, none);
        add(&mut km, ctrl('z'), Action::Undo, none);
        add(&mut km, ctrl('y'), Action::Redo, none);
        add(&mut km, ctrl('p'), Action::FuzzyFindFile, none);
        add(&mut km, ctrl_shift('p'), Action::ShowPalette, none);
        add(&mut km, ctrl('n'), Action::NewFile, none);
        // ctrl+w is used as a leader for split commands
        // ctrl+f is bound to PageDown above for normal mode
        add(&mut km, ctrl('o'), Action::OpenFile, none);

        add(&mut km, key(KeyCode::Tab, KeyModifiers::NONE), Action::NextBuffer, none);
        add(&mut km, key(KeyCode::Tab, KeyModifiers::SHIFT), Action::PreviousBuffer, none);
        add(&mut km, key(KeyCode::F(1), KeyModifiers::NONE), Action::ToggleFileTree, none);
        add(&mut km, ctrl('t'), Action::ToggleFileTree, none);
        add(&mut km, key(KeyCode::F(5), KeyModifiers::NONE), Action::ReloadFile, none);
        add(&mut km, key(KeyCode::F(11), KeyModifiers::NONE), Action::ToggleMarkdownPreview, none);
        add(&mut km, ctrl('k'), Action::ToggleMarkdownPreview, none);

        add(&mut km, alt_char('h'), Action::SplitHorizontal, none);
        add(&mut km, alt_char('v'), Action::SplitVertical, none);
        add(&mut km, alt_char('w'), Action::NextSplit, none);
        add(&mut km, alt_char('q'), Action::ClosePane, none);
        add(&mut km, alt_char('m'), Action::FocusPreview, none);

        // Terminal keybindings
        add(&mut km, key(KeyCode::F(12), KeyModifiers::NONE), Action::ToggleTerminal, none);
        add(&mut km, alt_char('t'), Action::ToggleTerminal, none);
        add(&mut km, ctrl('\\'), Action::FocusTerminal, none);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('t'), Action::ToggleTerminal, normal);

        // Git keybindings (g leader + key)
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('b'), Action::GitBlameToggle, normal);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('s'), Action::GitStatus, normal);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('r'), Action::GitBranchView, normal);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('a'), Action::GitStageFile, normal);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('u'), Action::GitUnstageFile, normal);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('c'), Action::GitCommit, normal);
        add_seq(&mut km, KeyCode::Char('g'), KeyCode::Char('d'), Action::GitDiff, normal);

        km
    }

    pub fn resolve(&mut self, key: KeyEvent, mode: Option<EditMode>) -> Option<Action> {
        let now = std::time::Instant::now();

        if let Some(last_time) = self.last_key_time {
            if now.duration_since(last_time) > self.timeout_duration {
                self.pending_keys.clear();
            }
        }

        if !self.pending_keys.is_empty() {
            let first = &self.pending_keys[0];
            let combo = (first.code, key.code, mode);
            if let Some(action) = self.sequence_bindings.get(&combo) {
                self.pending_keys.clear();
                self.last_key_time = None;
                return Some(action.clone());
            }
            let combo_none = (first.code, key.code, None);
            if let Some(action) = self.sequence_bindings.get(&combo_none) {
                self.pending_keys.clear();
                self.last_key_time = None;
                return Some(action.clone());
            }
            self.pending_keys.clear();
        }

        let direct = self.bindings.get(&(key, mode));
        if let Some(action) = direct {
            self.last_key_time = Some(now);
            return Some(action.clone());
        }

        let direct_none = self.bindings.get(&(key, None));
        if let Some(action) = direct_none {
            self.last_key_time = Some(now);
            return Some(action.clone());
        }

        if self.leader_keys.contains(&key.code) {
            self.pending_keys.push(key);
            self.last_key_time = Some(now);
            return None;
        }

        if key.code == KeyCode::Char('d') || key.code == KeyCode::Char('y') || key.code == KeyCode::Char('c') {
            let combo = (key.code, key.code, mode);
            if let Some(action) = self.sequence_bindings.get(&combo) {
                self.last_key_time = Some(now);
                return Some(action.clone());
            }
            let combo_none = (key.code, key.code, None);
            if let Some(action) = self.sequence_bindings.get(&combo_none) {
                self.last_key_time = Some(now);
                return Some(action.clone());
            }
        }

        self.last_key_time = Some(now);
        None
    }

    pub fn is_leader_key(&self, key: &KeyEvent) -> bool {
        self.leader_keys.contains(&key.code)
    }

    pub fn add_binding(&mut self, key: KeyEvent, action: Action, mode: Option<EditMode>) {
        self.bindings.insert((key, mode), action);
    }

    pub fn remove_binding(&mut self, key: KeyEvent, mode: Option<EditMode>) {
        self.bindings.remove(&(key, mode));
    }

    pub fn load_from_config(&mut self, config: &[(KeyEvent, Action, Option<EditMode>)]) {
        for (key, action, mode) in config {
            self.bindings.insert((*key, *mode), action.clone());
        }
    }

    pub fn reset_to_defaults(&mut self) {
        self.bindings = self.default_bindings.clone();
        self.sequence_bindings = self.default_sequence_bindings.clone();
        self.pending_keys.clear();
        self.last_key_time = None;
    }

    pub fn describe_binding(&self, action: &Action) -> Vec<String> {
        let mut result = Vec::new();
        for ((key, mode), act) in &self.bindings {
            if act == action {
                let key_str = format!("{:?}", key);
                let mode_str = match mode {
                    Some(m) => format!("{:?}", m),
                    None => "Any".to_string(),
                };
                result.push(format!("{} ({})", key_str, mode_str));
            }
        }
        for ((first, second, mode), act) in &self.sequence_bindings {
            if act == action {
                let key_str = format!("{:?} {:?}", first, second);
                let mode_str = match mode {
                    Some(m) => format!("{:?}", m),
                    None => "Any".to_string(),
                };
                result.push(format!("{} ({})", key_str, mode_str));
            }
        }
        result
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}


