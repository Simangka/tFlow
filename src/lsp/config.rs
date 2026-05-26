use crate::lsp::types::{ServerDefinition, default_server_definitions, LanguageId};
use std::collections::HashMap;

pub struct LanguageServerConfig {
    pub servers: HashMap<LanguageId, ServerDefinition>,
    pub file_language_map: HashMap<String, LanguageId>,
}

impl LanguageServerConfig {
    pub fn new() -> Self {
        let mut servers = HashMap::new();
        let mut file_language_map = HashMap::new();

        for def in default_server_definitions() {
            for lang in &def.language_ids {
                servers.insert(lang.clone(), def.clone());
            }
        }

        file_language_map.insert("rs".into(), "rust".into());
        file_language_map.insert("py".into(), "python".into());
        file_language_map.insert("js".into(), "javascript".into());
        file_language_map.insert("jsx".into(), "javascriptreact".into());
        file_language_map.insert("ts".into(), "typescript".into());
        file_language_map.insert("tsx".into(), "typescriptreact".into());
        file_language_map.insert("c".into(), "c".into());
        file_language_map.insert("h".into(), "c".into());
        file_language_map.insert("cpp".into(), "cpp".into());
        file_language_map.insert("cc".into(), "cpp".into());
        file_language_map.insert("cxx".into(), "cpp".into());
        file_language_map.insert("hpp".into(), "cpp".into());
        file_language_map.insert("go".into(), "go".into());
        file_language_map.insert("java".into(), "java".into());
        file_language_map.insert("rb".into(), "ruby".into());
        file_language_map.insert("php".into(), "php".into());
        file_language_map.insert("cs".into(), "csharp".into());
        file_language_map.insert("fs".into(), "fsharp".into());
        file_language_map.insert("swift".into(), "swift".into());
        file_language_map.insert("lua".into(), "lua".into());
        file_language_map.insert("kt".into(), "kotlin".into());
        file_language_map.insert("kts".into(), "kotlin".into());
        file_language_map.insert("zig".into(), "zig".into());
        file_language_map.insert("dart".into(), "dart".into());
        file_language_map.insert("elm".into(), "elm".into());
        file_language_map.insert("clj".into(), "clojure".into());
        file_language_map.insert("cljs".into(), "clojure".into());
        file_language_map.insert("ex".into(), "elixir".into());
        file_language_map.insert("exs".into(), "elixir".into());
        file_language_map.insert("hs".into(), "haskell".into());
        file_language_map.insert("sql".into(), "sql".into());
        file_language_map.insert("sh".into(), "shellscript".into());
        file_language_map.insert("bash".into(), "shellscript".into());
        file_language_map.insert("yaml".into(), "yaml".into());
        file_language_map.insert("yml".into(), "yaml".into());
        file_language_map.insert("json".into(), "json".into());
        file_language_map.insert("toml".into(), "toml".into());
        file_language_map.insert("md".into(), "markdown".into());
        file_language_map.insert("css".into(), "css".into());
        file_language_map.insert("html".into(), "html".into());
        file_language_map.insert("vue".into(), "vue".into());
        file_language_map.insert("svelte".into(), "svelte".into());
        file_language_map.insert("proto".into(), "proto".into());
        file_language_map.insert("graphql".into(), "graphql".into());
        file_language_map.insert("gql".into(), "graphql".into());

        Self { servers, file_language_map }
    }

    pub fn language_for_extension(&self, ext: &str) -> Option<&LanguageId> {
        self.file_language_map.get(ext)
    }

    pub fn server_for_language(&self, language: &str) -> Option<&ServerDefinition> {
        self.servers.get(language)
    }

    pub fn has_server_for(&self, language: &str) -> bool {
        self.servers.contains_key(language)
    }

    pub fn add_server(&mut self, language: LanguageId, def: ServerDefinition) {
        self.servers.insert(language, def);
    }

    pub fn add_file_mapping(&mut self, extension: String, language: LanguageId) {
        self.file_language_map.insert(extension, language);
    }
}

impl Default for LanguageServerConfig {
    fn default() -> Self {
        Self::new()
    }
}
