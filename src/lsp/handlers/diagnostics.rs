use crate::lsp::cache::LspCache;
use crate::lsp::types::*;
use lsp_types::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct PendingDiagnostics {
    pub doc_id: DocumentId,
    pub diagnostics: Vec<Diagnostic>,
    pub received_at: Instant,
}

pub struct DiagnosticsHandler {
    pending: HashMap<DocumentId, PendingDiagnostics>,
    debounce_ms: u64,
    last_emit: HashMap<DocumentId, Instant>,
    cache: Arc<LspCache>,
}

impl DiagnosticsHandler {
    pub fn new(cache: Arc<LspCache>, config: LspConfig) -> Self {
        Self {
            pending: HashMap::new(),
            debounce_ms: config.diagnostics_debounce_ms,
            last_emit: HashMap::new(),
            cache,
        }
    }

    pub fn push_diagnostics(&mut self, doc_id: DocumentId, diagnostics: Vec<Diagnostic>) {
        self.pending.insert(doc_id, PendingDiagnostics {
            doc_id,
            diagnostics,
            received_at: Instant::now(),
        });
    }

    pub fn ready_to_emit(&self) -> Vec<DocumentId> {
        let now = Instant::now();
        self.pending
            .keys()
            .filter(|&&doc_id| {
                if let Some(pending) = self.pending.get(&doc_id) {
                    let elapsed = now.duration_since(pending.received_at);
                    let since_last_emit = self.last_emit.get(&doc_id)
                        .map(|last| now.duration_since(*last))
                        .unwrap_or(Duration::MAX);
                    elapsed >= Duration::from_millis(self.debounce_ms) && since_last_emit >= Duration::from_millis(50)
                } else {
                    false
                }
            })
            .copied()
            .collect()
    }

    pub fn emit_ready(&mut self) -> Vec<LspEvent> {
        let ready = self.ready_to_emit();
        let mut events = Vec::with_capacity(ready.len());
        for doc_id in ready {
            if let Some(pending) = self.pending.remove(&doc_id) {
                self.cache.store_diagnostics(doc_id, pending.diagnostics.clone());
                self.last_emit.insert(doc_id, Instant::now());
                events.push(LspEvent::Diagnostics {
                    doc_id,
                    diagnostics: pending.diagnostics,
                });
            }
        }
        events
    }

    pub fn clear(&mut self, doc_id: DocumentId) {
        self.pending.remove(&doc_id);
        self.last_emit.remove(&doc_id);
        self.cache.diagnostics.remove(&doc_id);
    }

    pub fn has_pending(&self, doc_id: DocumentId) -> bool {
        self.pending.contains_key(&doc_id)
    }
}
