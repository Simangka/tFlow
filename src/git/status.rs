use std::path::Path;

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
                | git2::Status::INDEX_DELETED,
            );
            entries.push(StatusEntry { path, status: status_char(s), staged });
        }
    }

    // Workdir diff for unstaged hunks
    if let Ok(diff) = repo.diff_index_to_workdir(None, None) {
        for i in 0..diff.deltas().len() {
            if let Ok(patch) = git2::Patch::from_diff(&diff, i) {
                if let Some(patch) = patch {
                    let path = diff.deltas().nth(i)
                        .and_then(|d| d.new_file().path().map(|p| p.to_string_lossy().to_string()))
                        .unwrap_or_default();
                    if path.is_empty() { continue; }
                    let mut hunks = Vec::new();
                    for h in 0..patch.num_hunks() {
                        if let Ok((hunk, _)) = patch.hunk(h) {
                            let mut lines = Vec::new();
                            for l in 0..patch.num_lines_in_hunk(h).unwrap_or(0) {
                                if let Ok(line) = patch.line_in_hunk(h, l) {
                                    let content = String::from_utf8_lossy(line.content()).to_string();
                                    lines.push(content);
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
    }

    RepoStatus { entries, diffs }
}

pub fn fetch_diff(repo: &git2::Repository, file_path: &Path) -> Result<Vec<HunkInfo>, anyhow::Error> {
    let diff = repo.diff_index_to_workdir(None, None)?;
    let wd = repo.workdir().ok_or_else(|| anyhow::anyhow!("no workdir"))?;
    let relative = file_path.strip_prefix(wd)?;
    let mut result = Vec::new();
    for i in 0..diff.deltas().len() {
        let p = diff.deltas().nth(i).map(|d| d.new_file().path().map(|p| p.to_path_buf())).flatten();
        if p.as_deref() != Some(relative) { continue; }
        if let Ok(patch) = git2::Patch::from_diff(&diff, i) {
            if let Some(patch) = patch {
                for h in 0..patch.num_hunks() {
                    if let Ok((hunk, _)) = patch.hunk(h) {
                        let mut lines = Vec::new();
                        for l in 0..patch.num_lines_in_hunk(h).unwrap_or(0) {
                            if let Ok(line) = patch.line_in_hunk(h, l) {
                                lines.push(String::from_utf8_lossy(line.content()).to_string());
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
    }
    Ok(result)
}
