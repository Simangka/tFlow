use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use lsp_types::{Position as LspPosition, Diagnostic as LspDiagnostic, CompletionItem as LspCompletionItem};

pub type DocumentId = usize;
pub type LanguageId = String;
pub type RequestId = u64;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_request_id() -> RequestId {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone)]
pub enum LspCommand {
    StartServer {
        language: LanguageId,
        workspace_root: PathBuf,
    },
    StopServer {
        language: LanguageId,
    },
    DidOpen {
        doc_id: DocumentId,
        path: PathBuf,
        language_id: LanguageId,
        text: String,
        version: i32,
    },
    DidChange {
        doc_id: DocumentId,
        version: i32,
        changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
    },
    DidSave {
        doc_id: DocumentId,
    },
    DidClose {
        doc_id: DocumentId,
    },
    Completion {
        doc_id: DocumentId,
        position: LspPosition,
        trigger_kind: Option<lsp_types::CompletionTriggerKind>,
        trigger_character: Option<String>,
    },
    CompletionResolve {
        item: Box<LspCompletionItem>,
    },
    Hover {
        doc_id: DocumentId,
        position: LspPosition,
    },
    GotoDefinition {
        doc_id: DocumentId,
        position: LspPosition,
    },
    References {
        doc_id: DocumentId,
        position: LspPosition,
        include_declaration: bool,
    },
    SemanticTokens {
        doc_id: DocumentId,
        range: Option<lsp_types::Range>,
    },
}

#[derive(Debug, Clone)]
pub enum LspEvent {
    ServerStarted {
        language: LanguageId,
        capabilities: Option<lsp_types::ServerCapabilities>,
    },
    ServerStopped {
        language: LanguageId,
        reason: String,
    },
    ServerError {
        language: LanguageId,
        error: String,
    },
    Diagnostics {
        doc_id: DocumentId,
        diagnostics: Vec<LspDiagnostic>,
    },
    CompletionResult {
        doc_id: DocumentId,
        items: Vec<LspCompletionItem>,
        is_incomplete: bool,
    },
    CompletionResolveResult {
        item: Box<LspCompletionItem>,
    },
    HoverResult {
        doc_id: DocumentId,
        contents: Option<lsp_types::Hover>,
    },
    GotoDefinitionResult {
        doc_id: DocumentId,
        locations: Vec<lsp_types::LocationLink>,
    },
    ReferencesResult {
        doc_id: DocumentId,
        locations: Vec<lsp_types::Location>,
    },
    SemanticTokensResult {
        doc_id: DocumentId,
        tokens: Vec<lsp_types::SemanticToken>,
    },
}

#[derive(Debug, Clone)]
pub struct LspConfig {
    pub max_completion_items: usize,
    pub completion_debounce_ms: u64,
    pub diagnostics_debounce_ms: u64,
    pub hover_delay_ms: u64,
    pub enable_semantic_tokens: bool,
    pub enable_diagnostics: bool,
    pub enable_completion: bool,
    pub enable_hover: bool,
    pub enable_goto_definition: bool,
    pub enable_references: bool,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            max_completion_items: 100,
            completion_debounce_ms: 80,
            diagnostics_debounce_ms: 200,
            hover_delay_ms: 300,
            enable_semantic_tokens: true,
            enable_diagnostics: true,
            enable_completion: true,
            enable_hover: true,
            enable_goto_definition: true,
            enable_references: true,
        }
    }
}

pub fn char_offset_to_utf16(line: &str, char_offset: usize) -> u32 {
    line.chars()
        .take(char_offset)
        .map(|c| c.len_utf16() as u32)
        .sum()
}

pub fn utf16_to_char_offset(line: &str, utf16_offset: u32) -> usize {
    let mut so_far = 0u32;
    for (i, c) in line.chars().enumerate() {
        if so_far >= utf16_offset {
            return i;
        }
        so_far += c.len_utf16() as u32;
    }
    line.chars().count()
}

pub fn lsp_position_from_editor(line: usize, col: usize, line_text: &str) -> LspPosition {
    LspPosition::new(line as u32, char_offset_to_utf16(line_text, col))
}

pub fn path_to_uri(path: &PathBuf) -> lsp_types::Url {
    // Canonicalize to get absolute path, since Url::from_file_path requires absolute paths.
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
    lsp_types::Url::from_file_path(&abs).expect("valid path after canonicalization")
}

pub fn uri_to_path(uri: &lsp_types::Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

#[derive(Debug, Clone)]
pub struct ServerDefinition {
    pub command: String,
    pub args: Vec<String>,
    pub language_ids: Vec<String>,
    pub initialization_options: Option<serde_json::Value>,
    pub settings: Option<serde_json::Value>,
}

pub fn default_server_definitions() -> Vec<ServerDefinition> {
    vec![
        ServerDefinition {
            command: "rust-analyzer".into(),
            args: vec![],
            language_ids: vec!["rust".into()],
            initialization_options: Some(serde_json::json!({
                "checkOnSave": true,
                "cargo": { "allFeatures": true }
            })),
            settings: None,
        },
        ServerDefinition {
            command: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            language_ids: vec!["javascript".into(), "typescript".into(), "javascriptreact".into(), "typescriptreact".into()],
            initialization_options: None,
            settings: None,
        },
        ServerDefinition {
            command: "pyright-langserver".into(),
            args: vec!["--stdio".into()],
            language_ids: vec!["python".into()],
            initialization_options: None,
            settings: None,
        },
        ServerDefinition {
            command: "clangd".into(),
            args: vec![],
            language_ids: vec!["c".into(), "cpp".into()],
            initialization_options: None,
            settings: None,
        },
    ]
}

pub fn language_id_for_path(path: &PathBuf) -> Option<(String, LanguageId)> {
    let ext = path.extension()?.to_str()?;
    let (lang, lang_id) = match ext {
        "rs" => ("rust", "rust"),
        "py" => ("python", "python"),
        "js" => ("javascript", "javascript"),
        "jsx" => ("javascriptreact", "javascriptreact"),
        "ts" => ("typescript", "typescript"),
        "tsx" => ("typescriptreact", "typescriptreact"),
        "c" => ("c", "c"),
        "h" => ("c", "c"),
        "cpp" | "cc" | "cxx" => ("cpp", "cpp"),
        "hpp" | "hh" | "hxx" => ("cpp", "cpp"),
        "go" => ("go", "go"),
        "mod" => ("go", "go"),
        "sum" => ("go", "go"),
        "zig" => ("zig", "zig"),
        "java" => ("java", "java"),
        "kt" | "kts" => ("kotlin", "kotlin"),
        "rb" => ("ruby", "ruby"),
        "php" => ("php", "php"),
        "cs" => ("csharp", "csharp"),
        "fs" | "fsx" => ("fsharp", "fsharp"),
        "swift" => ("swift", "swift"),
        "lua" => ("lua", "lua"),
        "sh" | "bash" => ("shellscript", "shellscript"),
        "yaml" | "yml" => ("yaml", "yaml"),
        "json" => ("json", "json"),
        "toml" => ("toml", "toml"),
        "md" | "markdown" => ("markdown", "markdown"),
        "css" => ("css", "css"),
        "scss" | "sass" => ("scss", "scss"),
        "html" => ("html", "html"),
        "vue" => ("vue", "vue"),
        "svelte" => ("svelte", "svelte"),
        "dart" => ("dart", "dart"),
        "elm" => ("elm", "elm"),
        "clj" | "cljs" | "cljc" | "edn" => ("clojure", "clojure"),
        "ex" | "exs" => ("elixir", "elixir"),
        "erl" | "hrl" => ("erlang", "erlang"),
        "hs" | "lhs" => ("haskell", "haskell"),
        "ml" | "mli" => ("ocaml", "ocaml"),
        "nim" => ("nim", "nim"),
        "r" => ("r", "r"),
        "sql" => ("sql", "sql"),
        "proto" => ("proto", "proto"),
        "graphql" | "gql" => ("graphql", "graphql"),
        _ => return None,
    };
    Some((lang_id.to_string(), lang.to_string()))
}
