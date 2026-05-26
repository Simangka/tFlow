use crate::lsp::cache::LspCache;
use crate::lsp::types::*;
use lsp_types::*;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CompletionHandler {
    generation: AtomicU64,
    last_request_id: AtomicU64,
    config: LspConfig,
}

impl CompletionHandler {
    pub fn new(config: LspConfig) -> Self {
        Self {
            generation: AtomicU64::new(0),
            last_request_id: AtomicU64::new(0),
            config,
        }
    }

    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst)
    }

    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub fn is_stale(&self, generation: u64) -> bool {
        generation != self.generation.load(Ordering::SeqCst)
    }

    pub fn mark_request_sent(&self, request_id: RequestId) {
        self.last_request_id.store(request_id, Ordering::SeqCst);
    }

    pub fn last_request(&self) -> RequestId {
        self.last_request_id.load(Ordering::SeqCst)
    }

    pub fn process_completion_response(
        &self,
        cache: &LspCache,
        doc_id: DocumentId,
        result: serde_json::Value,
        generation: u64,
    ) -> Option<(Vec<CompletionItem>, bool)> {
        if self.is_stale(generation) {
            return None;
        }

        let response: CompletionResponse = match serde_json::from_value(result) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse completion response");
                return None;
            }
        };

        let (items, is_incomplete) = match response {
            CompletionResponse::List(list) => {
                let items: Vec<CompletionItem> = list.items.into_iter()
                    .take(self.config.max_completion_items)
                    .collect();
                (items, list.is_incomplete)
            }
            CompletionResponse::Array(arr) => {
                let items: Vec<CompletionItem> = arr.into_iter()
                    .take(self.config.max_completion_items)
                    .collect();
                (items, false)
            }
        };

        cache.store_completions(doc_id, items.clone(), is_incomplete, generation);

        Some((items, is_incomplete))
    }
}
