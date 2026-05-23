use std::path::{Path, PathBuf};

pub fn discover(repo: &git2::Repository, file_path: &Path) -> Result<PathBuf, anyhow::Error> {
    let wd = repo.workdir().ok_or_else(|| anyhow::anyhow!("no workdir"))?;
    let relative = file_path.strip_prefix(wd)?;
    Ok(relative.to_path_buf())
}

#[derive(Debug, Clone)]
pub struct BlameInfo {
    pub line: usize,
    pub author: String,
    pub time: i64,
}

pub fn fetch_blame(repo: &git2::Repository, file_path: &Path) -> Result<Vec<BlameInfo>, anyhow::Error> {
    let relative = discover(repo, file_path)?;
    let mut opts = git2::BlameOptions::new();
    let blame = repo.blame_file(&relative, Some(&mut opts))?;
    let mut results = Vec::new();
    for hunk in blame.iter() {
        let final_line = hunk.final_start_line() as usize;
        let lines = hunk.lines_in_hunk() as usize;
        let author = hunk.final_signature().name().unwrap_or("Unknown").to_string();
        let time = hunk.final_signature().when().seconds();
        for offset in 0..lines {
            results.push(BlameInfo {
                line: final_line + offset - 1,
                author: author.clone(),
                time,
            });
        }
    }
    results.sort_by_key(|b| b.line);
    Ok(results)
}
