use crate::lsp::types::DocumentId;
use dashmap::DashMap;
use lsp_types::{CompletionItem, Diagnostic, SemanticToken};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CachedDiagnostics {
    pub diagnostics: Arc<Vec<Diagnostic>>,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct CachedCompletion {
    pub items: Arc<Vec<CompletionItem>>,
    pub is_incomplete: bool,
    pub request_id: u64,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone)]
pub struct CachedSemanticTokens {
    pub tokens: Arc<Vec<SemanticToken>>,
    pub version: u64,
}

#[derive(Debug)]
pub struct LspCache {
    pub diagnostics: DashMap<DocumentId, CachedDiagnostics>,
    pub completions: DashMap<DocumentId, CachedCompletion>,
    pub semantic_tokens: DashMap<DocumentId, CachedSemanticTokens>,
    next_version: std::sync::atomic::AtomicU64,
}

impl LspCache {
    pub fn new() -> Self {
        Self {
            diagnostics: DashMap::new(),
            completions: DashMap::new(),
            semantic_tokens: DashMap::new(),
            next_version: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub fn store_diagnostics(&self, doc_id: DocumentId, diags: Vec<Diagnostic>) {
        let version = self.next_version.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.diagnostics.insert(doc_id, CachedDiagnostics {
            diagnostics: Arc::new(diags),
            version,
        });
    }

    pub fn get_diagnostics(&self, doc_id: DocumentId) -> Option<CachedDiagnostics> {
        self.diagnostics.get(&doc_id).map(|d| d.clone())
    }

    pub fn store_completions(&self, doc_id: DocumentId, items: Vec<CompletionItem>, is_incomplete: bool, request_id: u64) {
        self.completions.insert(doc_id, CachedCompletion {
            items: Arc::new(items),
            is_incomplete,
            request_id,
            timestamp: std::time::Instant::now(),
        });
    }

    pub fn get_completions(&self, doc_id: DocumentId) -> Option<CachedCompletion> {
        self.completions.get(&doc_id).map(|c| c.clone())
    }

    pub fn clear_completions(&self, doc_id: DocumentId) {
        self.completions.remove(&doc_id);
    }

    pub fn store_semantic_tokens(&self, doc_id: DocumentId, tokens: Vec<SemanticToken>) {
        let version = self.next_version.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.semantic_tokens.insert(doc_id, CachedSemanticTokens {
            tokens: Arc::new(tokens),
            version,
        });
    }

    pub fn get_semantic_tokens(&self, doc_id: DocumentId) -> Option<CachedSemanticTokens> {
        self.semantic_tokens.get(&doc_id).map(|t| t.clone())
    }

    pub fn remove_document(&self, doc_id: DocumentId) {
        self.diagnostics.remove(&doc_id);
        self.completions.remove(&doc_id);
        self.semantic_tokens.remove(&doc_id);
    }

    pub fn has_diagnostics(&self, doc_id: DocumentId) -> bool {
        self.diagnostics.contains_key(&doc_id)
    }
}

impl Default for LspCache {
    fn default() -> Self {
        Self::new()
    }
}
