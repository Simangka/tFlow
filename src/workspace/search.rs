use std::path::{Path, PathBuf};
use regex::Regex;
use ignore::WalkBuilder;
use crate::workspace::SearchResult;

#[derive(Debug, Clone)]
pub struct WorkspaceSearcher {
    pub root: PathBuf,
    pub query: String,
    pub case_sensitive: bool,
    pub is_regex: bool,
    pub max_results: usize,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub results: Vec<SearchResult>,
    pub is_searching: bool,
    pub progress: f32,
}

impl WorkspaceSearcher {
    pub fn new(root: PathBuf) -> Self {
        WorkspaceSearcher {
            root,
            query: String::new(),
            case_sensitive: false,
            is_regex: false,
            max_results: 500,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            results: Vec::new(),
            is_searching: false,
            progress: 0.0,
        }
    }

    pub fn search(&mut self) -> Result<(), anyhow::Error> {
        self.is_searching = true;
        self.progress = 0.0;
        self.results.clear();

        if self.query.is_empty() {
            self.is_searching = false;
            self.progress = 1.0;
            return Ok(());
        }

        let query = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        let regex = if self.is_regex {
            let pattern = if self.case_sensitive {
                self.query.clone()
            } else {
                format!("(?i){}", self.query)
            };
            Some(Regex::new(&pattern).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?)
        } else {
            None
        };

        let include_regexes: Vec<Regex> = self.include_patterns.iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();
        let exclude_regexes: Vec<Regex> = self.exclude_patterns.iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        let mut processed = 0usize;

        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(true)
            .build();

        let file_paths: Vec<PathBuf> = walker
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect();

        let total_files = file_paths.len();

        for file_path in &file_paths {
            if self.results.len() >= self.max_results {
                break;
            }

            let should_include = if include_regexes.is_empty() {
                true
            } else {
                let path_str = file_path.to_string_lossy();
                include_regexes.iter().any(|r| r.is_match(&path_str))
            };

            if !should_include {
                processed += 1;
                self.progress = if total_files > 0 { processed as f32 / total_files as f32 } else { 1.0 };
                continue;
            }

            let should_exclude = if exclude_regexes.is_empty() {
                false
            } else {
                let path_str = file_path.to_string_lossy();
                exclude_regexes.iter().any(|r| r.is_match(&path_str))
            };

            if should_exclude {
                processed += 1;
                self.progress = if total_files > 0 { processed as f32 / total_files as f32 } else { 1.0 };
                continue;
            }

            match search_file(file_path, &query, &regex, self.case_sensitive) {
                Ok(matches) => {
                    for m in &matches {
                        if self.results.len() >= self.max_results {
                            break;
                        }
                        self.results.push(m.clone());
                    }
                    if self.results.len() >= self.max_results {
                        break;
                    }
                }
                Err(_) => {}
            }

            processed += 1;
            self.progress = if total_files > 0 { processed as f32 / total_files as f32 } else { 1.0 };
        }

        self.progress = 1.0;
        self.is_searching = false;
        Ok(())
    }

    pub fn search_async(&self) -> tokio::task::JoinHandle<Result<Vec<SearchResult>, anyhow::Error>> {
        let mut searcher = self.clone();
        tokio::task::spawn_blocking(move || {
            searcher.search()?;
            Ok(searcher.results)
        })
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
    }

    pub fn clear_results(&mut self) {
        self.results.clear();
        self.progress = 0.0;
        self.is_searching = false;
    }

    pub fn cancel(&mut self) {
        self.is_searching = false;
    }

    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    pub fn open_result(&self, index: usize) -> Option<(PathBuf, usize, usize)> {
        self.results.get(index).map(|r| {
            (r.path.clone(), r.line, r.column)
        })
    }
}

fn search_file(
    path: &Path,
    query: &str,
    regex: &Option<Regex>,
    case_sensitive: bool,
) -> Result<Vec<SearchResult>, anyhow::Error> {
    let content = std::fs::read_to_string(path)?;
    let mut results = Vec::new();

    if let Some(re) = regex {
        for (line_idx, line) in content.lines().enumerate() {
            for m in re.find_iter(line) {
                results.push(SearchResult {
                    path: path.to_path_buf(),
                    line: line_idx,
                    column: m.start(),
                    line_content: line.to_string(),
                    match_start: m.start(),
                    match_end: m.end(),
                });
            }
        }
    } else {
        for (line_idx, line) in content.lines().enumerate() {
            let search_line = if case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            let mut start = 0;
            while let Some(pos) = search_line[start..].find(query) {
                let abs_pos = start + pos;
                results.push(SearchResult {
                    path: path.to_path_buf(),
                    line: line_idx,
                    column: abs_pos,
                    line_content: line.to_string(),
                    match_start: abs_pos,
                    match_end: abs_pos + query.len(),
                });
                start = abs_pos + 1;
                if start >= search_line.len() {
                    break;
                }
            }
        }
    }

    Ok(results)
}
