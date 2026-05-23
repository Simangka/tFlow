pub fn stage_file(repo: &git2::Repository, path: &str) -> Result<(), anyhow::Error> {
    let mut index = repo.index()?;
    index.add_path(std::path::Path::new(path))?;
    index.write()?;
    Ok(())
}

pub fn unstage_file(repo: &git2::Repository, path: &str) -> Result<(), anyhow::Error> {
    let tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let exists_in_head = tree.as_ref().and_then(|t| t.get_path(std::path::Path::new(path)).ok()).is_some();
    if exists_in_head {
        // File exists in HEAD — reset index entry to match HEAD
        if let Some(h) = repo.head().ok() {
            if let Some(target) = h.target() {
                if let Ok(obj) = repo.find_object(target, None) {
                    let _ = repo.reset_default(Some(&obj), &[std::path::Path::new(path)]);
                    return Ok(());
                }
            }
        }
    }
    // New file that was staged — remove from index entirely
    let mut index = repo.index()?;
    index.remove_path(std::path::Path::new(path))?;
    index.write()?;
    Ok(())
}

pub fn stage_hunk(_repo: &git2::Repository, _path: &str, _hunk_idx: usize) -> Result<(), anyhow::Error> {
    Ok(())
}

pub fn unstage_hunk(_repo: &git2::Repository, _path: &str, _hunk_idx: usize) -> Result<(), anyhow::Error> {
    Ok(())
}
