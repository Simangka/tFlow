use std::path::Path;

fn validate_relative_path(path: &str) -> Result<&str, anyhow::Error> {
    if path.contains("..") || path.starts_with('/') || path.starts_with('\\') {
        anyhow::bail!("invalid path");
    }
    Ok(path)
}

pub fn stage_file(repo: &git2::Repository, path: &str) -> Result<(), anyhow::Error> {
    let path = validate_relative_path(path)?;
    let mut index = repo.index()?;
    index.add_path(Path::new(path))?;
    index.write()?;
    Ok(())
}

pub fn unstage_file(repo: &git2::Repository, path: &str) -> Result<(), anyhow::Error> {
    let path = validate_relative_path(path)?;
    let head = repo.head().ok();
    let tree = head.as_ref().and_then(|h| h.peel_to_tree().ok());
    let exists_in_head = tree.as_ref()
        .and_then(|t| t.get_path(Path::new(path)).ok())
        .is_some();
    if exists_in_head {
        let head = head.ok_or_else(|| anyhow::anyhow!("no HEAD"))?;
        let head_commit = head.peel_to_commit()?;
        let head_tree = head_commit.tree()?;
        let obj = head_tree.get_path(Path::new(path))?.to_object(&repo)?;
        repo.reset_default(Some(&obj), &[Path::new(path)])?;
    } else {
        let mut index = repo.index()?;
        index.remove_path(Path::new(path))?;
        index.write()?;
    }
    Ok(())
}

pub fn stage_hunk(_repo: &git2::Repository, _path: &str, _hunk_idx: usize) -> Result<(), anyhow::Error> {
    Err(anyhow::anyhow!("hunk staging not implemented"))
}

pub fn unstage_hunk(_repo: &git2::Repository, _path: &str, _hunk_idx: usize) -> Result<(), anyhow::Error> {
    Err(anyhow::anyhow!("hunk staging not implemented"))
}
