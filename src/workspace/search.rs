use std::path::{Path, PathBuf};
use regex::Regex;
use ignore::WalkBuilder;
use crate::workspace::SearchResult;

const MAX_SEARCH_FILE_SIZE: u64 = 10 * 1024 * 1024;
const SKIP_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "bin", "png", "jpg", "jpeg", "zip", "tar", "gz", "bz2", "xz", "7z",
    "lock", "ico", "gif", "bmp", "pdf", "mp3", "mp4", "mov", "avi", "mkv", "webp", "wasm",
    "o", "a", "lib", "obj", "class", "jar", "pyc", "pyd", "ttf", "otf", "woff", "woff2",
    "iso", "img", "deb", "rpm", "dmg", "swf", "flv", "ogg", "wav", "flac", "aac",
];

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

        let query_lower: Option<String> = if self.case_sensitive {
            None
        } else {
            Some(self.query.to_lowercase())
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
            .follow_links(false)
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

            if should_skip_by_extension(file_path) {
                processed += 1;
                self.progress = if total_files > 0 { processed as f32 / total_files as f32 } else { 1.0 };
                continue;
            }

            if let Ok(md) = std::fs::metadata(file_path) {
                if md.len() > MAX_SEARCH_FILE_SIZE {
                    processed += 1;
                    self.progress = if total_files > 0 { processed as f32 / total_files as f32 } else { 1.0 };
                    continue;
                }
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

            match search_file(file_path, &self.query, query_lower.as_deref(), &regex, self.case_sensitive) {
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

fn should_skip_by_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        if SKIP_EXTENSIONS.contains(&ext_lower.as_str()) {
            return true;
        }
    }
    false
}

fn search_file(
    path: &Path,
    query: &str,
    query_lower: Option<&str>,
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
    } else if case_sensitive {
        for (line_idx, line) in content.lines().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(query) {
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
                if start >= line.len() {
                    break;
                }
            }
        }
    } else {
        let needle_lower = match query_lower {
            Some(q) => q,
            None => return Ok(results),
        };
        for (line_idx, line) in content.lines().enumerate() {
            let matches = find_case_insensitive(line, &needle_lower);
            for (byte_start, byte_end) in matches {
                results.push(SearchResult {
                    path: path.to_path_buf(),
                    line: line_idx,
                    column: byte_start,
                    line_content: line.to_string(),
                    match_start: byte_start,
                    match_end: byte_end,
                });
            }
        }
    }

    Ok(results)
}

fn find_case_insensitive(haystack: &str, needle_lower: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    if needle_lower.is_empty() {
        return results;
    }
    let needle_chars: Vec<char> = needle_lower.chars().collect();
    let hay_chars: Vec<char> = haystack.chars().collect();
    let mut byte_indices: Vec<usize> = Vec::with_capacity(hay_chars.len() + 1);
    {
        let mut b = 0usize;
        for c in &hay_chars {
            byte_indices.push(b);
            b += c.len_utf8();
        }
        byte_indices.push(b);
    }
    'outer: for start in 0..hay_chars.len() {
        let mut ni = 0usize;
        let mut hi = start;
        while ni < needle_chars.len() && hi < hay_chars.len() {
            let hlc: Vec<char> = hay_chars[hi].to_lowercase().collect();
            if ni + hlc.len() > needle_chars.len() {
                continue 'outer;
            }
            if needle_chars[ni..ni + hlc.len()] != hlc[..] {
                continue 'outer;
            }
            ni += hlc.len();
            hi += 1;
        }
        if ni == needle_chars.len() {
            let byte_start = byte_indices[start];
            let byte_end = byte_indices[hi];
            results.push((byte_start, byte_end));
        }
    }
    results
}
