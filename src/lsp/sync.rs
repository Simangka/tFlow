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
            let start = self.lsp_to_rope_pos(range.start);
            let end = self.lsp_to_rope_pos(range.end);
            let start_idx = self.rope.line_to_char(start.0) + start.1;
            let end_idx = self.rope.line_to_char(end.0) + end.1;
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

    fn lsp_to_rope_pos(&self, pos: lsp_types::Position) -> (usize, usize) {
        let line = pos.line as usize;
        let col = pos.character as usize;
        if line >= self.rope.len_lines() {
            return (self.rope.len_lines().saturating_sub(1), 0);
        }
        let line_text = self.rope.line(line).to_string();
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
