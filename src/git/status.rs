use std::path::Path;

const MAX_LINES_PER_FILE: usize = 10_000;
const MAX_LINE_BYTES: usize = 4096;

#[derive(Debug, Clone)]
pub struct StatusEntry {
    pub path: String,
    pub status: char,
    pub staged: bool,
}

#[derive(Debug, Clone)]
pub struct HunkInfo {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub header: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: String,
    pub status: char,
    pub hunks: Vec<HunkInfo>,
}

#[derive(Debug, Clone)]
pub struct RepoStatus {
    pub entries: Vec<StatusEntry>,
    pub diffs: Vec<FileDiff>,
}

fn status_char(s: git2::Status) -> char {
    if s.contains(git2::Status::INDEX_MODIFIED) { 'M' }
    else if s.contains(git2::Status::INDEX_NEW) { 'A' }
    else if s.contains(git2::Status::INDEX_DELETED) { 'D' }
    else if s.contains(git2::Status::INDEX_RENAMED) { 'R' }
    else if s.contains(git2::Status::INDEX_TYPECHANGE) { 'T' }
    else if s.contains(git2::Status::WT_MODIFIED) { 'm' }
    else if s.contains(git2::Status::WT_NEW) { '?' }
    else if s.contains(git2::Status::WT_DELETED) { 'd' }
    else if s.contains(git2::Status::WT_RENAMED) { 'r' }
    else if s.contains(git2::Status::WT_TYPECHANGE) { 't' }
    else { ' ' }
}

fn truncate_line(content: &str) -> String {
    if content.len() <= MAX_LINE_BYTES {
        content.to_string()
    } else {
        let mut s = content[..MAX_LINE_BYTES].to_string();
        s.push('\n');
        s
    }
}

pub fn fetch_status(repo: &git2::Repository) -> RepoStatus {
    let mut entries = Vec::new();
    let mut diffs = Vec::new();

    if let Ok(statuses) = repo.statuses(Some(
        git2::StatusOptions::new()
            .include_untracked(true)
            .recurse_untracked_dirs(true),
    )) {
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("").to_string();
            if path.is_empty() { continue; }
            let s = entry.status();
            let staged = s.intersects(
                git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_NEW
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE,
            );
            entries.push(StatusEntry { path, status: status_char(s), staged });
        }
    }

    if let Ok(diff) = repo.diff_index_to_workdir(None, None) {
        let deltas: Vec<_> = diff.deltas().collect();
        for i in 0..deltas.len() {
            let delta = &deltas[i];
            if delta.flags().is_binary() { continue; }
            if let Ok(Some(patch)) = git2::Patch::from_diff(&diff, i) {
                let path = delta.new_file().path()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                if path.is_empty() { continue; }
                let mut hunks = Vec::new();
                for h in 0..patch.num_hunks() {
                    if let Ok((hunk, _)) = patch.hunk(h) {
                        let mut lines = Vec::new();
                        let num_lines = patch.num_lines_in_hunk(h).unwrap_or(0).min(MAX_LINES_PER_FILE);
                        for l in 0..num_lines {
                            if let Ok(line) = patch.line_in_hunk(h, l) {
                                let content = String::from_utf8_lossy(line.content());
                                lines.push(truncate_line(&content));
                            }
                        }
                        let hdr = String::from_utf8_lossy(hunk.header()).to_string();
                        hunks.push(HunkInfo {
                            old_start: hunk.old_start() as usize,
                            old_lines: hunk.old_lines() as usize,
                            new_start: hunk.new_start() as usize,
                            new_lines: hunk.new_lines() as usize,
                            header: hdr,
                            lines,
                        });
                    }
                }
                let ch = match patch.delta().status() {
                    git2::Delta::Modified => 'm',
                    git2::Delta::Added => '?',
                    git2::Delta::Deleted => 'd',
                    _ => ' ',
                };
                diffs.push(FileDiff { path, status: ch, hunks });
            }
        }
    }

    RepoStatus { entries, diffs }
}

pub fn fetch_diff(repo: &git2::Repository, file_path: &Path) -> Result<Vec<HunkInfo>, anyhow::Error> {
    let diff = repo.diff_index_to_workdir(None, None)?;
    let wd = repo.workdir().ok_or_else(|| anyhow::anyhow!("no workdir"))?;
    let wd_canon = std::fs::canonicalize(wd).unwrap_or_else(|_| wd.to_path_buf());
    let file_canon = std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    let relative = file_canon.strip_prefix(&wd_canon)?;
    let mut result = Vec::new();
    let deltas: Vec<_> = diff.deltas().collect();
    for i in 0..deltas.len() {
        let delta = &deltas[i];
        if delta.flags().is_binary() { continue; }
        let p = delta.new_file().path();
        if p.as_deref() != Some(relative) { continue; }
        if let Ok(Some(patch)) = git2::Patch::from_diff(&diff, i) {
            for h in 0..patch.num_hunks() {
                if let Ok((hunk, _)) = patch.hunk(h) {
                    let mut lines = Vec::new();
                    let num_lines = patch.num_lines_in_hunk(h).unwrap_or(0).min(MAX_LINES_PER_FILE);
                    for l in 0..num_lines {
                        if let Ok(line) = patch.line_in_hunk(h, l) {
                            let content = String::from_utf8_lossy(line.content());
                            lines.push(truncate_line(&content));
                        }
                    }
                    result.push(HunkInfo {
                        old_start: hunk.old_start() as usize,
                        old_lines: hunk.old_lines() as usize,
                        new_start: hunk.new_start() as usize,
                        new_lines: hunk.new_lines() as usize,
                        header: String::from_utf8_lossy(hunk.header()).to_string(),
                        lines,
                    });
                }
            }
        }
    }
    Ok(result)
}
