use crate::lsp::rpc::{RpcWriter, JsonRpcError, RpcMessage};
use crate::lsp::sync::{DocumentSnapshot, DocumentSyncState};
use crate::lsp::types::*;
use lsp_types::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::mpsc;
use tokio::time::Instant;

#[derive(Debug)]
struct PendingRequest {
    method: String,
    doc_id: Option<DocumentId>,
    sent_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientState {
    Starting,
    Initializing,
    Initialized,
    Shutdown,
    Crashed(String),
}

pub struct LanguageClient {
    pub language: LanguageId,
    pub state: ClientState,
    pub server_capabilities: Option<ServerCapabilities>,

    pub writer: Option<RpcWriter>,
    document_states: HashMap<DocumentId, DocumentSyncState>,
    pending_requests: HashMap<RequestId, PendingRequest>,
    workspace_root: Option<PathBuf>,

    request_id_gen: Arc<AtomicU64>,
    event_tx: mpsc::UnboundedSender<LspEvent>,
}

impl LanguageClient {
    pub fn new(
        language: LanguageId,
        event_tx: mpsc::UnboundedSender<LspEvent>,
    ) -> Self {
        Self {
            language,
            state: ClientState::Starting,
            server_capabilities: None,
            writer: None,
            document_states: HashMap::new(),
            pending_requests: HashMap::new(),
            workspace_root: None,
            request_id_gen: Arc::new(AtomicU64::new(1)),
            event_tx,
        }
    }

    pub fn set_writer(&mut self, writer: RpcWriter) {
        self.writer = Some(writer);
    }

    pub fn has_capability(&self, check: impl FnOnce(&ServerCapabilities) -> bool) -> bool {
        self.server_capabilities.as_ref().map_or(false, check)
    }

    pub async fn send_initialize(
        &mut self,
        workspace_root: Option<PathBuf>,
        init_options: Option<serde_json::Value>,
    ) -> Result<(), String> {
        self.state = ClientState::Initializing;
        self.workspace_root = workspace_root;

        let writer = self.writer.as_mut().ok_or("No writer")?;

        let workspace_folders = self.workspace_root.as_ref().map(|root| {
            vec![WorkspaceFolder {
                uri: path_to_uri(root),
                name: root.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "root".into()),
            }]
        });

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            client_info: Some(ClientInfo {
                name: "tflow".into(),
                version: Some("0.1.0".into()),
            }),
            locale: None,
            root_path: self.workspace_root.as_ref().map(|p| p.to_string_lossy().to_string()),
            root_uri: self.workspace_root.as_ref().map(path_to_uri),
            initialization_options: init_options,
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    synchronization: Some(TextDocumentSyncClientCapabilities {
                        dynamic_registration: Some(false),
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                        did_save: Some(true),
                    }),
                    completion: Some(CompletionClientCapabilities {
                        dynamic_registration: Some(false),
                        completion_item: Some(CompletionItemCapability {
                            snippet_support: Some(true),
                            commit_characters_support: Some(true),
                            documentation_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                            ..Default::default()
                        }),
                        completion_item_kind: Some(CompletionItemKindCapability {
                            value_set: Some(vec![
                                CompletionItemKind::TEXT,
                                CompletionItemKind::METHOD,
                                CompletionItemKind::FUNCTION,
                                CompletionItemKind::CONSTRUCTOR,
                                CompletionItemKind::FIELD,
                                CompletionItemKind::VARIABLE,
                                CompletionItemKind::CLASS,
                                CompletionItemKind::INTERFACE,
                                CompletionItemKind::MODULE,
                                CompletionItemKind::PROPERTY,
                                CompletionItemKind::UNIT,
                                CompletionItemKind::VALUE,
                                CompletionItemKind::ENUM,
                                CompletionItemKind::KEYWORD,
                                CompletionItemKind::SNIPPET,
                                CompletionItemKind::COLOR,
                                CompletionItemKind::FILE,
                                CompletionItemKind::REFERENCE,
                                CompletionItemKind::CONSTANT,
                                CompletionItemKind::STRUCT,
                                CompletionItemKind::EVENT,
                                CompletionItemKind::OPERATOR,
                                CompletionItemKind::TYPE_PARAMETER,
                            ]),
                        }),
                        context_support: Some(true),
                        insert_text_mode: Some(InsertTextMode::AS_IS),
                        ..Default::default()
                    }),
                    hover: Some(HoverClientCapabilities {
                        dynamic_registration: Some(false),
                        content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                    }),
                    definition: Some(GotoCapability {
                        dynamic_registration: Some(false),
                        link_support: Some(true),
                    }),
                    references: Some(ReferenceClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    document_symbol: Some(DocumentSymbolClientCapabilities {
                        dynamic_registration: Some(false),
                        symbol_kind: Some(SymbolKindCapability {
                            value_set: Some(vec![
                                SymbolKind::FILE, SymbolKind::MODULE, SymbolKind::NAMESPACE,
                                SymbolKind::PACKAGE, SymbolKind::CLASS, SymbolKind::METHOD,
                                SymbolKind::PROPERTY, SymbolKind::FIELD, SymbolKind::CONSTRUCTOR,
                                SymbolKind::ENUM, SymbolKind::INTERFACE, SymbolKind::FUNCTION,
                                SymbolKind::VARIABLE, SymbolKind::CONSTANT, SymbolKind::STRING,
                                SymbolKind::NUMBER, SymbolKind::BOOLEAN, SymbolKind::ARRAY,
                                SymbolKind::OBJECT, SymbolKind::KEY, SymbolKind::NULL,
                                SymbolKind::ENUM_MEMBER, SymbolKind::STRUCT, SymbolKind::EVENT,
                                SymbolKind::OPERATOR, SymbolKind::TYPE_PARAMETER,
                            ]),
                        }),
                        hierarchical_document_symbol_support: Some(true),
                        ..Default::default()
                    }),
                    semantic_tokens: Some(SemanticTokensClientCapabilities {
                        requests: SemanticTokensClientCapabilitiesRequests {
                            range: Some(true),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                        token_types: vec![],
                        token_modifiers: vec![],
                        formats: vec![TokenFormat::RELATIVE],
                        ..Default::default()
                    }),
                    publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                        related_information: Some(false),
                        tag_support: Some(TagSupport {
                            value_set: vec![DiagnosticTag::UNNECESSARY, DiagnosticTag::DEPRECATED],
                        }),
                        version_support: Some(false),
                        code_description_support: Some(false),
                        data_support: Some(false),
                    }),
                    ..Default::default()
                }),
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    show_message: Some(ShowMessageRequestClientCapabilities {
                        message_action_item: Some(MessageActionItemCapabilities {
                            additional_properties_support: Some(true),
                        }),
                    }),
                    show_document: Some(ShowDocumentClientCapabilities {
                        support: false,
                    }),
                    ..Default::default()
                }),
                general: Some(GeneralClientCapabilities {
                    position_encodings: Some(vec![PositionEncodingKind::UTF16]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            trace: Some(TraceValue::Verbose),
            workspace_folders,
            ..Default::default()
        };

        let id = writer.send_request(
            "initialize",
            Some(serde_json::to_value(params).unwrap()),
        ).await.map_err(|e| e.to_string())?;

        self.pending_requests.insert(id, PendingRequest {
            method: "initialize".into(),
            doc_id: None,
            sent_at: Instant::now(),
        });

        Ok(())
    }

    pub async fn handle_message(&mut self, msg: RpcMessage) -> Result<(), String> {
        match msg {
            RpcMessage::Response { id, result, error } => {
                self.handle_response(id, result, error).await
            }
            RpcMessage::Notification { method, params } => {
                self.handle_notification(&method, params).await
            }
            RpcMessage::Request { id, method, params } => {
                self.handle_server_request(id, &method, params).await
            }
        }
    }

    async fn handle_response(
        &mut self,
        id: RequestId,
        result: Option<serde_json::Value>,
        error: Option<JsonRpcError>,
    ) -> Result<(), String> {
        let pending = self.pending_requests.remove(&id);
        let method = pending.as_ref().map(|p| p.method.as_str()).unwrap_or("unknown");

        if let Some(ref err) = error {
            tracing::warn!(language = %self.language, method = %method, error = %err, "Request failed");
            if method == "initialize" {
                self.state = ClientState::Crashed(err.message.clone());
                let _ = self.event_tx.send(LspEvent::ServerError {
                    language: self.language.clone(),
                    error: err.message.clone(),
                });
            }
            return Ok(());
        }

        match method {
            "initialize" => {
                if let Some(ref res) = result {
                    let init_result: InitializeResult = serde_json::from_value(res.clone())
                        .map_err(|e| format!("Failed to parse InitializeResult: {}", e))?;
                    let caps = init_result.capabilities.clone();
                    self.server_capabilities = Some(caps.clone());

                    if let Some(ref mut writer) = self.writer {
                        let _ = writer.send_notification("initialized", Some(serde_json::json!({}))).await;
                    }

                    self.state = ClientState::Initialized;

                    let _ = self.event_tx.send(LspEvent::ServerStarted {
                        language: self.language.clone(),
                        capabilities: Some(caps),
                    });

                    let doc_ids: Vec<DocumentId> = self.document_states.keys().copied().collect();
                    for doc_id in doc_ids {
                        self.send_did_open(doc_id).await;
                    }
                }
            }
            "shutdown" => {
                self.state = ClientState::Shutdown;
                if let Some(ref mut writer) = self.writer {
                    let _ = writer.send_notification("exit", None).await;
                }
            }
            "textDocument/completion" => {
                let doc_id = pending.as_ref().and_then(|p| p.doc_id);
                match (doc_id, &result) {
                    (Some(did), Some(res)) => {
                        match serde_json::from_value::<Option<CompletionResponse>>(res.clone()) {
                            Ok(Some(comp)) => {
                                let (items, is_incomplete) = match comp {
                                    CompletionResponse::Array(items) => (items, false),
                                    CompletionResponse::List(list) => (list.items, list.is_incomplete),
                                };
                                let _ = self.event_tx.send(LspEvent::CompletionResult {
                                    doc_id: did,
                                    items,
                                    is_incomplete,
                                });
                            }
                            Ok(None) => {}
                            Err(_) => {}
                        }
                    }
                    _ => {}
                }
            }
            "textDocument/hover" => {
                let doc_id = pending.as_ref().and_then(|p| p.doc_id);
                if let (Some(did), Some(ref res)) = (doc_id, result) {
                    if let Ok(hover) = serde_json::from_value::<Option<Hover>>(res.clone()) {
                        let _ = self.event_tx.send(LspEvent::HoverResult {
                            doc_id: did,
                            contents: hover,
                        });
                    }
                }
            }
            "textDocument/definition" | "textDocument/declaration" => {
                let doc_id = pending.as_ref().and_then(|p| p.doc_id);
                if let (Some(did), Some(ref res)) = (doc_id, result) {
                    if let Ok(locations) = serde_json::from_value::<Option<GotoDefinitionResponse>>(res.clone()) {
                        let links = match locations {
                            Some(GotoDefinitionResponse::Scalar(loc)) => vec![LocationLink {
                                origin_selection_range: None,
                                target_uri: loc.uri,
                                target_range: loc.range,
                                target_selection_range: loc.range,
                            }],
                            Some(GotoDefinitionResponse::Array(locs)) => locs.into_iter().map(|loc| LocationLink {
                                origin_selection_range: None,
                                target_uri: loc.uri,
                                target_range: loc.range,
                                target_selection_range: loc.range,
                            }).collect(),
                            Some(GotoDefinitionResponse::Link(links)) => links,
                            None => vec![],
                        };
                        let _ = self.event_tx.send(LspEvent::GotoDefinitionResult {
                            doc_id: did,
                            locations: links,
                        });
                    }
                }
            }
            "textDocument/references" => {
                let doc_id = pending.as_ref().and_then(|p| p.doc_id);
                if let (Some(did), Some(ref res)) = (doc_id, result) {
                    if let Ok(locations) = serde_json::from_value::<Option<Vec<Location>>>(res.clone()) {
                        let _ = self.event_tx.send(LspEvent::ReferencesResult {
                            doc_id: did,
                            locations: locations.unwrap_or_default(),
                        });
                    }
                }
            }
            "textDocument/semanticTokens/full" => {
                let doc_id = pending.as_ref().and_then(|p| p.doc_id);
                if let (Some(did), Some(ref res)) = (doc_id, result) {
                    if let Ok(tokens) = serde_json::from_value::<Option<SemanticTokens>>(res.clone()) {
                        let _ = self.event_tx.send(LspEvent::SemanticTokensResult {
                            doc_id: did,
                            tokens: tokens.map(|t| t.data).unwrap_or_default(),
                        });
                    }
                }
            }
            other => {
                tracing::debug!(language = %self.language, method = %other, "Unhandled response");
            }
        }

        Ok(())
    }

    async fn handle_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), String> {
        match method {
            "textDocument/publishDiagnostics" => {
                if let Some(ref p) = params {
                    let diag_params: PublishDiagnosticsParams = serde_json::from_value(p.clone())
                        .map_err(|e| format!("Bad diagnostics: {}", e))?;
                    let doc_id = self.resolve_doc_id(&diag_params.uri);
                    if let Some(did) = doc_id {
                        let _ = self.event_tx.send(LspEvent::Diagnostics {
                            doc_id: did,
                            diagnostics: diag_params.diagnostics,
                        });
                    }
                }
            }
            "window/showMessage" => {
                if let Some(ref p) = params {
                    let msg: ShowMessageParams = serde_json::from_value(p.clone())
                        .map_err(|e| format!("Bad showMessage: {}", e))?;
                    tracing::info!(language = %self.language, type = ?msg.typ, message = %msg.message, "Server message");
                }
            }
            "window/logMessage" => {
                if let Some(ref p) = params {
                    let msg: LogMessageParams = serde_json::from_value(p.clone())
                        .map_err(|e| format!("Bad logMessage: {}", e))?;
                    tracing::debug!(language = %self.language, type = ?msg.typ, message = %msg.message, "Server log");
                }
            }
            other => {
                tracing::debug!(language = %self.language, method = %other, "Unhandled notification");
            }
        }
        Ok(())
    }

    async fn handle_server_request(
        &mut self,
        id: RequestId,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), String> {
        let writer = self.writer.as_mut().ok_or("No writer")?;
        match method {
            "window/workDoneProgress/create" => {
                writer.send_response(id, Some(serde_json::json!(null)), None).await
                    .map_err(|e| e.to_string())?;
            }
            "client/registerCapability" => {
                writer.send_response(id, Some(serde_json::json!(null)), None).await
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                writer.send_response(id, None, Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", method),
                    data: None,
                })).await.map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn resolve_doc_id(&self, uri: &Url) -> Option<DocumentId> {
        for (id, state) in &self.document_states {
            if &state.uri == uri {
                return Some(*id);
            }
        }
        None
    }

    pub fn get_document(&self, doc_id: DocumentId) -> Option<&DocumentSyncState> {
        self.document_states.get(&doc_id)
    }

    fn get_document_mut(&mut self, doc_id: DocumentId) -> Option<&mut DocumentSyncState> {
        self.document_states.get_mut(&doc_id)
    }

    pub fn snapshot(&self, doc_id: DocumentId) -> Option<DocumentSnapshot> {
        self.document_states.get(&doc_id).map(|s| s.snapshot())
    }

    pub fn push_document_state(&mut self, doc_id: DocumentId, state: DocumentSyncState) {
        self.document_states.insert(doc_id, state);
    }

    pub async fn send_did_open(&mut self, doc_id: DocumentId) {
        let state_copy = match self.document_states.get(&doc_id) {
            Some(s) => (
                s.uri.clone(),
                s.language_id.clone(),
                s.version,
                s.current_text(),
            ),
            None => return,
        };
        if self.state == ClientState::Initialized {
            let writer = match self.writer.as_mut() {
                Some(w) => w,
                None => return,
            };
            let params = DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: state_copy.0,
                    language_id: state_copy.1,
                    version: state_copy.2,
                    text: state_copy.3,
                },
            };
            let _ = writer.send_notification(
                "textDocument/didOpen",
                Some(serde_json::to_value(params).unwrap()),
            ).await;
        }
    }

    pub async fn send_did_change(&mut self, doc_id: DocumentId, version: i32, changes: Vec<TextDocumentContentChangeEvent>) {
        let state = match self.get_document_mut(doc_id) {
            Some(s) => s,
            None => return,
        };
        state.apply_changes(&changes);
        let state_uri = state.uri.clone();
        let state_version = state.version;

        if self.state != ClientState::Initialized {
            return;
        }

        let writer = match self.writer.as_mut() {
            Some(w) => w,
            None => return,
        };

        let text_doc = VersionedTextDocumentIdentifier {
            uri: state_uri,
            version: state_version,
        };

        let params = DidChangeTextDocumentParams {
            text_document: text_doc,
            content_changes: changes,
        };
        let _ = writer.send_notification(
            "textDocument/didChange",
            Some(serde_json::to_value(params).unwrap()),
        ).await;
    }

    pub async fn send_did_save(&mut self, doc_id: DocumentId) {
        let uri = match self.get_document(doc_id) {
            Some(s) => s.uri.clone(),
            None => return,
        };
        if self.state != ClientState::Initialized {
            return;
        }
        let writer = match self.writer.as_mut() {
            Some(w) => w,
            None => return,
        };
        let text_doc = TextDocumentIdentifier { uri };
        let params = DidSaveTextDocumentParams {
            text_document: text_doc,
            text: None,
        };
        let _ = writer.send_notification(
            "textDocument/didSave",
            Some(serde_json::to_value(params).unwrap()),
        ).await;
    }

    pub async fn send_did_close(&mut self, doc_id: DocumentId) {
        let state = match self.document_states.remove(&doc_id) {
            Some(s) => s,
            None => return,
        };
        if self.state != ClientState::Initialized {
            return;
        }
        let writer = match self.writer.as_mut() {
            Some(w) => w,
            None => return,
        };
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: state.uri },
        };
        let _ = writer.send_notification(
            "textDocument/didClose",
            Some(serde_json::to_value(params).unwrap()),
        ).await;
    }

    pub async fn send_completion(
        &mut self,
        doc_id: DocumentId,
        position: Position,
        trigger_kind: Option<CompletionTriggerKind>,
        trigger_character: Option<String>,
        request_id: RequestId,
    ) -> Result<(), String> {
        if self.state != ClientState::Initialized {
            return Ok(());
        }
        let uri = self.get_document(doc_id).map(|s| s.uri.clone()).ok_or("Document not found")?;
        let writer = self.writer.as_mut().ok_or("No writer")?;
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: trigger_kind.map(|kind| CompletionContext {
                trigger_kind: kind,
                trigger_character,
            }),
        };
        let id = writer.send_request(
            "textDocument/completion",
            Some(serde_json::to_value(params).unwrap()),
        ).await.map_err(|e| e.to_string())?;
        self.pending_requests.insert(id, PendingRequest {
            method: "textDocument/completion".into(),
            doc_id: Some(doc_id),
            sent_at: Instant::now(),
        });
        Ok(())
    }

    pub async fn send_hover(
        &mut self,
        doc_id: DocumentId,
        position: Position,
    ) -> Result<(), String> {
        if self.state != ClientState::Initialized {
            return Ok(());
        }
        let uri = self.get_document(doc_id).map(|s| s.uri.clone()).ok_or("Document not found")?;
        let writer = self.writer.as_mut().ok_or("No writer")?;
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        let id = writer.send_request(
            "textDocument/hover",
            Some(serde_json::to_value(params).unwrap()),
        ).await.map_err(|e| e.to_string())?;
        self.pending_requests.insert(id, PendingRequest {
            method: "textDocument/hover".into(),
            doc_id: Some(doc_id),
            sent_at: Instant::now(),
        });
        Ok(())
    }

    pub async fn send_goto_definition(
        &mut self,
        doc_id: DocumentId,
        position: Position,
    ) -> Result<(), String> {
        if self.state != ClientState::Initialized {
            return Ok(());
        }
        let uri = self.get_document(doc_id).map(|s| s.uri.clone()).ok_or("Document not found")?;
        let writer = self.writer.as_mut().ok_or("No writer")?;
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        let id = writer.send_request(
            "textDocument/definition",
            Some(serde_json::to_value(params).unwrap()),
        ).await.map_err(|e| e.to_string())?;
        self.pending_requests.insert(id, PendingRequest {
            method: "textDocument/definition".into(),
            doc_id: Some(doc_id),
            sent_at: Instant::now(),
        });
        Ok(())
    }

    pub async fn send_references(
        &mut self,
        doc_id: DocumentId,
        position: Position,
        include_declaration: bool,
    ) -> Result<(), String> {
        if self.state != ClientState::Initialized {
            return Ok(());
        }
        let uri = self.get_document(doc_id).map(|s| s.uri.clone()).ok_or("Document not found")?;
        let writer = self.writer.as_mut().ok_or("No writer")?;
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };
        let id = writer.send_request(
            "textDocument/references",
            Some(serde_json::to_value(params).unwrap()),
        ).await.map_err(|e| e.to_string())?;
        self.pending_requests.insert(id, PendingRequest {
            method: "textDocument/references".into(),
            doc_id: Some(doc_id),
            sent_at: Instant::now(),
        });
        Ok(())
    }

    pub async fn send_semantic_tokens_full(
        &mut self,
        doc_id: DocumentId,
    ) -> Result<(), String> {
        if self.state != ClientState::Initialized {
            return Ok(());
        }
        let uri = self.get_document(doc_id).map(|s| s.uri.clone()).ok_or("Document not found")?;
        let writer = self.writer.as_mut().ok_or("No writer")?;
        let params = SemanticTokensParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        let id = writer.send_request(
            "textDocument/semanticTokens/full",
            Some(serde_json::to_value(params).unwrap()),
        ).await.map_err(|e| e.to_string())?;
        self.pending_requests.insert(id, PendingRequest {
            method: "textDocument/semanticTokens".into(),
            doc_id: Some(doc_id),
            sent_at: Instant::now(),
        });
        Ok(())
    }

    pub fn cancel_request(&mut self, method: &str) {
        self.pending_requests.retain(|id, req| {
            if req.method == method {
                if let Some(ref mut writer) = self.writer {
                    let cancel_params = CancelParams {
                        id: NumberOrString::Number(*id as i32),
                    };
                    let _ = writer.send_notification(
                        "$/cancelRequest",
                        Some(serde_json::to_value(cancel_params).unwrap()),
                    );
                }
                false
            } else {
                true
            }
        });
    }

    pub fn cancel_all_completions(&mut self) {
        self.cancel_request("textDocument/completion");
    }

    pub fn cancel_all_hover(&mut self) {
        self.cancel_request("textDocument/hover");
    }

    pub fn has_pending_completion(&self) -> bool {
        self.pending_requests.values().any(|r| r.method == "textDocument/completion")
    }

    pub fn has_pending_request(&self, method: &str) -> bool {
        self.pending_requests.values().any(|r| r.method == method)
    }

    pub async fn send_shutdown(&mut self) {
        if self.state != ClientState::Initialized {
            return;
        }
        let writer = match self.writer.as_mut() {
            Some(w) => w,
            None => return,
        };
        if let Ok(id) = writer.send_request("shutdown", None).await {
            self.pending_requests.insert(id, PendingRequest {
                method: "shutdown".into(),
                doc_id: None,
                sent_at: Instant::now(),
            });
        }
    }

    pub fn document_count(&self) -> usize {
        self.document_states.len()
    }

    pub fn has_document(&self, doc_id: DocumentId) -> bool {
        self.document_states.contains_key(&doc_id)
    }
}
