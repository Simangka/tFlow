use crate::lsp::cache::LspCache;
use crate::lsp::client::LanguageClient;
use crate::lsp::config::LanguageServerConfig;
use crate::lsp::handlers::{CompletionHandler, DiagnosticsHandler};
use crate::lsp::rpc::{RpcReader, RpcMessage};
use crate::lsp::types::*;
use lsp_types::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::Duration;

struct ServerConnection {
    client: LanguageClient,
    reader_rx: mpsc::UnboundedReceiver<RpcMessage>,
    process: tokio::process::Child,
}

pub struct LspManager {
    servers: HashMap<LanguageId, ServerConnection>,
    server_for_document: HashMap<DocumentId, LanguageId>,
    config: LanguageServerConfig,
    #[allow(dead_code)]
    lsp_config: LspConfig,
    cache: Arc<LspCache>,
    completion_handler: CompletionHandler,
    diagnostics_handler: DiagnosticsHandler,
    event_tx: mpsc::UnboundedSender<LspEvent>,
    cmd_rx: mpsc::UnboundedReceiver<LspCommand>,
}

impl LspManager {
    pub fn new(
        cmd_rx: mpsc::UnboundedReceiver<LspCommand>,
        event_tx: mpsc::UnboundedSender<LspEvent>,
        config: LanguageServerConfig,
        lsp_config: LspConfig,
    ) -> Self {
        let cache = Arc::new(LspCache::new());
        let diagnostics_handler = DiagnosticsHandler::new(Arc::clone(&cache), lsp_config.clone());
        let completion_handler = CompletionHandler::new(lsp_config.clone());

        Self {
            servers: HashMap::new(),
            server_for_document: HashMap::new(),
            config,
            lsp_config,
            cache,
            completion_handler,
            diagnostics_handler,
            event_tx,
            cmd_rx,
        }
    }

    pub async fn run(&mut self) {
        let mut debounce_timer = tokio::time::interval(Duration::from_millis(50));

        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => {
                            if let Err(e) = self.handle_command(cmd).await {
                                tracing::error!(error = %e, "Command handling error");
                            }
                        }
                        None => {
                            tracing::info!("LSP command channel closed, shutting down");
                            break;
                        }
                    }
                }
                _ = debounce_timer.tick() => {
                    self.process_debounced().await;
                    self.drain_reader_messages().await;
                }
            }
        }

        for (_, mut conn) in self.servers.drain() {
            conn.client.send_shutdown().await;
            let _ = conn.process.kill().await;
        }
    }

    async fn drain_reader_messages(&mut self) {
        for (_lang, conn) in &mut self.servers {
            loop {
                match conn.reader_rx.try_recv() {
                    Ok(msg) => {
                        let _ = conn.client.handle_message(msg).await;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        tracing::warn!(language = %_lang, "Reader channel disconnected");
                        break;
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: LspCommand) -> Result<(), String> {
        match cmd {
            LspCommand::StartServer { language, workspace_root } => {
                self.start_server(&language, Some(workspace_root)).await
            }
            LspCommand::StopServer { language } => {
                self.stop_server(&language).await
            }
            LspCommand::DidOpen { doc_id, path, language_id, text, version } => {
                self.handle_did_open(doc_id, path, language_id, text, version).await
            }
            LspCommand::DidChange { doc_id, version, changes } => {
                self.handle_did_change(doc_id, version, changes).await
            }
            LspCommand::DidSave { doc_id } => {
                self.handle_did_save(doc_id).await
            }
            LspCommand::DidClose { doc_id } => {
                self.handle_did_close(doc_id).await
            }
            LspCommand::Completion { doc_id, position, trigger_kind, trigger_character } => {
                self.handle_completion(doc_id, position, trigger_kind, trigger_character).await
            }
            LspCommand::CompletionResolve { .. } => Ok(()),
            LspCommand::Hover { doc_id, position } => {
                self.handle_hover(doc_id, position).await
            }
            LspCommand::GotoDefinition { doc_id, position } => {
                self.handle_goto_definition(doc_id, position).await
            }
            LspCommand::References { doc_id, position, include_declaration } => {
                self.handle_references(doc_id, position, include_declaration).await
            }
            LspCommand::SemanticTokens { doc_id, range } => {
                self.handle_semantic_tokens(doc_id, range).await
            }
        }
    }

    async fn start_server(&mut self, language: &str, workspace_root: Option<PathBuf>) -> Result<(), String> {
        if self.servers.contains_key(language) {
            return Ok(());
        }

        let server_def = self.config.server_for_language(language)
            .ok_or_else(|| format!("No server config for language: {}", language))?
            .clone();

        tracing::info!(language = %language, "Starting server: {} {}", server_def.command, server_def.args.join(" "));

        let mut cmd = if cfg!(windows) {
            // On Windows, .cmd wrappers don't work well with piped stdio.
            // Resolve to the actual executable if possible.
            let (exe, args) = resolve_windows_command(&server_def.command, &server_def.args);
            let mut c = Command::new(&exe);
            c.args(&args);
            c
        } else {
            let mut c = Command::new(&server_def.command);
            c.args(&server_def.args);
            c
        };
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(ref root) = workspace_root {
            cmd.current_dir(root);
        }

        let mut child = cmd.spawn().map_err(|e| {
            format!("Failed to start '{}': {}", server_def.command, e)
        })?;

        let stdin = child.stdin.take().ok_or("No stdin")?;
        let stdout = child.stdout.take().ok_or("No stdout")?;
        let stderr = child.stderr.take().ok_or("No stderr")?;

        let writer = crate::lsp::rpc::RpcWriter::new(stdin);
        let reader = RpcReader::new(stdout);

        let (reader_tx, reader_rx) = mpsc::unbounded_channel();
        let lang = language.to_string();
        let lang2 = lang.clone();

        tokio::spawn(async move {
            crate::lsp::rpc::read_stderr(stderr, lang).await;
        });

        tokio::spawn(async move {
            let mut reader = reader;
            loop {
                match reader.read_message().await {
                    Ok(Some(msg)) => {
                        if reader_tx.send(msg).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        tracing::info!(language = %lang2, "Server connection closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!(language = %lang2, error = %e, "Read error");
                        break;
                    }
                }
            }
        });

        let mut client = LanguageClient::new(
            language.to_string(),
            self.event_tx.clone(),
        );
        client.set_writer(writer);

        client.send_initialize(workspace_root, server_def.initialization_options.clone()).await?;

        self.servers.insert(language.to_string(), ServerConnection {
            client,
            reader_rx,
            process: child,
        });

        Ok(())
    }

    async fn stop_server(&mut self, language: &str) -> Result<(), String> {
        if let Some(mut conn) = self.servers.remove(language) {
            conn.client.send_shutdown().await;
            let _ = conn.process.kill().await;
            let _ = conn.process.wait().await;
            tracing::info!(language = %language, "Server stopped");
        }
        Ok(())
    }

    async fn ensure_server(&mut self, language: &str, workspace_root: Option<PathBuf>) -> Result<(), String> {
        if !self.servers.contains_key(language) {
            self.start_server(language, workspace_root).await?;
        }
        Ok(())
    }

    fn get_client_mut(&mut self, doc_id: DocumentId) -> Option<&mut LanguageClient> {
        let lang = self.server_for_document.get(&doc_id)?.clone();
        self.servers.get_mut(&lang).map(|conn| &mut conn.client)
    }

    async fn handle_did_open(&mut self, doc_id: DocumentId, path: PathBuf, language_id: String, text: String, _version: i32) -> Result<(), String> {
        // Register doc -> language mapping BEFORE ensure_server so get_client_mut works
        self.server_for_document.insert(doc_id, language_id.clone());

        // Try to start the server if not already running; ignore errors so subsequent
        // commands can still find the doc_id -> language mapping.
        let workspace_root = path.parent().map(|p| p.to_path_buf());
        if let Err(e) = self.ensure_server(&language_id, workspace_root).await {
            tracing::warn!(error = %e, language = %language_id, "Failed to start LSP server");
        }

        if let Some(conn) = self.servers.get_mut(&language_id) {
            let state = crate::lsp::sync::DocumentSyncState::new(doc_id, path, language_id, text);
            conn.client.push_document_state(doc_id, state);
            conn.client.send_did_open(doc_id).await;
        }
        Ok(())
    }

    async fn handle_did_change(&mut self, doc_id: DocumentId, _version: i32, changes: Vec<TextDocumentContentChangeEvent>) -> Result<(), String> {
        self.drain_reader_messages().await;
        if let Some(client) = self.get_client_mut(doc_id) {
            let version = client.get_document(doc_id)
                .map(|d| d.version + 1)
                .unwrap_or(1);
            client.send_did_change(doc_id, version, changes).await;
        }
        Ok(())
    }

    async fn handle_did_save(&mut self, doc_id: DocumentId) -> Result<(), String> {
        if let Some(client) = self.get_client_mut(doc_id) {
            client.send_did_save(doc_id).await;
        }
        Ok(())
    }

    async fn handle_did_close(&mut self, doc_id: DocumentId) -> Result<(), String> {
        if let Some(client) = self.get_client_mut(doc_id) {
            client.send_did_close(doc_id).await;
        }
        self.server_for_document.remove(&doc_id);
        self.cache.remove_document(doc_id);
        self.diagnostics_handler.clear(doc_id);
        Ok(())
    }

    async fn handle_completion(
        &mut self,
        doc_id: DocumentId,
        position: Position,
        trigger_kind: Option<CompletionTriggerKind>,
        trigger_character: Option<String>,
    ) -> Result<(), String> {
        // Drain reader messages first so initialize response is processed
        // before we try to send a completion request.
        self.drain_reader_messages().await;

        self.completion_handler.next_generation();
        let request_id = crate::lsp::types::next_request_id();

        let client = match self.get_client_mut(doc_id) {
            Some(c) => c,
            None => return Ok(()),
        };

        client.cancel_all_completions();
        client.send_completion(doc_id, position, trigger_kind, trigger_character, request_id).await
    }

    async fn handle_hover(&mut self, doc_id: DocumentId, position: Position) -> Result<(), String> {
        let client = match self.get_client_mut(doc_id) {
            Some(c) => c,
            None => return Ok(()),
        };
        client.cancel_all_hover();
        client.send_hover(doc_id, position).await
    }

    async fn handle_goto_definition(&mut self, doc_id: DocumentId, position: Position) -> Result<(), String> {
        let client = match self.get_client_mut(doc_id) {
            Some(c) => c,
            None => return Ok(()),
        };
        client.send_goto_definition(doc_id, position).await
    }

    async fn handle_references(&mut self, doc_id: DocumentId, position: Position, include_declaration: bool) -> Result<(), String> {
        let client = match self.get_client_mut(doc_id) {
            Some(c) => c,
            None => return Ok(()),
        };
        client.send_references(doc_id, position, include_declaration).await
    }

    async fn handle_semantic_tokens(&mut self, doc_id: DocumentId, _range: Option<Range>) -> Result<(), String> {
        let client = match self.get_client_mut(doc_id) {
            Some(c) => c,
            None => return Ok(()),
        };
        client.send_semantic_tokens_full(doc_id).await
    }

    async fn process_debounced(&mut self) {
        let events = self.diagnostics_handler.emit_ready();
        for event in events {
            let _ = self.event_tx.send(event);
        }
    }

    pub fn cache(&self) -> &LspCache {
        &self.cache
    }
}

pub async fn run_lsp_manager(
    cmd_rx: mpsc::UnboundedReceiver<LspCommand>,
    event_tx: mpsc::UnboundedSender<LspEvent>,
    config: Option<LanguageServerConfig>,
    lsp_config: Option<LspConfig>,
) {
    let config = config.unwrap_or_default();
    let lsp_config = lsp_config.unwrap_or_default();
    let mut manager = LspManager::new(cmd_rx, event_tx, config, lsp_config);
    manager.run().await;
}

/// On Windows, .cmd wrappers in PATH may not work with piped stdio in tokio.
/// Try to find the actual executable by reading the .cmd file.
#[cfg(windows)]
fn resolve_windows_command(command: &str, original_args: &[String]) -> (String, Vec<String>) {
    // Use where.exe to find the full path
    let output = std::process::Command::new("where.exe")
        .arg(command)
        .output()
        .ok();
    if let Some(out) = output {
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("").to_string();
            let path = if path.ends_with(".cmd") || path.ends_with(".bat") {
                path
            } else {
                // where.exe might return a script without extension;
                // try appending .cmd
                let with_cmd = format!("{}.cmd", path);
                if std::path::Path::new(&with_cmd).exists() {
                    with_cmd
                } else {
                    path
                }
            };
            if path.ends_with(".cmd") || path.ends_with(".bat") {
                if let Some(dir) = std::path::Path::new(&path).parent() {
                    let candidates = [
                        dir.join("node_modules").join("pyright").join("langserver.index.js"),
                        dir.join("node_modules").join("pyright").join("dist").join("pyright-langserver.js"),
                    ];
                    for script in &candidates {
                        if script.exists() {
                            let mut args = vec![script.to_string_lossy().to_string()];
                            args.extend(original_args.iter().cloned());
                            return ("node".to_string(), args);
                        }
                    }
                }
            }
        }
    }
    // Fallback: return the original command unchanged
    (command.to_string(), original_args.to_vec())
}

#[cfg(not(windows))]
fn resolve_windows_command(command: &str, original_args: &[String]) -> (String, Vec<String>) {
    (command.to_string(), original_args.to_vec())
}
