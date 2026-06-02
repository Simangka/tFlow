use crate::lsp::types::{path_to_uri, DocumentId};
use lsp_types::{TextDocumentContentChangeEvent, Url, VersionedTextDocumentIdentifier};
use ropey::Rope;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub uri: Url,
    pub version: i32,
    pub language_id: String,
    pub text: String,
}

impl DocumentSnapshot {
    pub fn text_document_identifier(&self) -> VersionedTextDocumentIdentifier {
        VersionedTextDocumentIdentifier {
            uri: self.uri.clone(),
            version: self.version,
        }
    }
}

#[derive(Debug)]
pub struct DocumentSyncState {
    pub doc_id: DocumentId,
    pub uri: Url,
    pub path: PathBuf,
    pub language_id: String,
    pub version: i32,
    rope: Rope,
}

impl DocumentSyncState {
    pub fn new(doc_id: DocumentId, path: PathBuf, language_id: String, text: String) -> Self {
        let uri = path_to_uri(&path);
        Self {
            doc_id,
            uri,
            path,
            language_id,
            version: 1,
            rope: Rope::from_str(&text),
        }
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            uri: self.uri.clone(),
            version: self.version,
            language_id: self.language_id.clone(),
            text: self.rope.to_string(),
        }
    }

    pub fn apply_change(&mut self, change: &TextDocumentContentChangeEvent) {
        self.version += 1;
        if let Some(range) = change.range {
            let mut start_idx = self.lsp_to_rope_idx(range.start);
            let mut end_idx = self.lsp_to_rope_idx(range.end);
            let len = self.rope.len_chars();
            if start_idx > len {
                start_idx = len;
            }
            if end_idx > len {
                end_idx = len;
            }
            if end_idx > start_idx {
                self.rope.remove(start_idx..end_idx);
            }
            self.rope.insert(start_idx, &change.text);
        } else {
            self.rope = Rope::from_str(&change.text);
        }
    }

    pub fn apply_changes(&mut self, changes: &[TextDocumentContentChangeEvent]) {
        for change in changes {
            self.apply_change(change);
        }
    }

    pub fn replace_text(&mut self, text: String) {
        self.version += 1;
        self.rope = Rope::from_str(&text);
    }

    fn lsp_to_rope_idx(&self, pos: lsp_types::Position) -> usize {
        let line_count = self.rope.len_lines();
        let line = (pos.line as usize).min(line_count.saturating_sub(1));
        let line_text = self.rope.line(line).to_string();
        let line_chars = line_text.chars().count();
        let col = (pos.character as usize).min(line_chars);
        let rope_col = crate::lsp::types::utf16_to_char_offset(&line_text, col as u32);
        self.rope.line_to_char(line) + rope_col
    }

    fn lsp_to_rope_pos(&self, pos: lsp_types::Position) -> (usize, usize) {
        let line_count = self.rope.len_lines();
        let line = (pos.line as usize).min(line_count.saturating_sub(1));
        let line_text = self.rope.line(line).to_string();
        let max_col = line_text.chars().count();
        let col = (pos.character as usize).min(max_col);
        let rope_col = crate::lsp::types::utf16_to_char_offset(&line_text, col as u32);
        (line, rope_col)
    }

    pub fn current_text(&self) -> String {
        self.rope.to_string()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn text_document_identifier(&self) -> VersionedTextDocumentIdentifier {
        VersionedTextDocumentIdentifier {
            uri: self.uri.clone(),
            version: self.version,
        }
    }
}

pub fn build_text_content_change(
    old_text: &str,
    new_text: &str,
    range: lsp_types::Range,
) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: Some(range),
        range_length: None,
        text: new_text.to_string(),
    }
}

pub fn empty_document_identifier(uri: Url) -> VersionedTextDocumentIdentifier {
    VersionedTextDocumentIdentifier { uri, version: 1 }
}
