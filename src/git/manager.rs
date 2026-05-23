use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crate::git::blame::{fetch_blame, BlameInfo};
use crate::git::status::{fetch_status, RepoStatus};
use crate::git::operations::{stage_file, unstage_file};

pub struct GitManager {
    repos: HashMap<PathBuf, git2::Repository>,
    blames: HashMap<PathBuf, Vec<BlameInfo>>,
    statuses: HashMap<PathBuf, RepoStatus>,
    branches: HashMap<PathBuf, String>,
}

impl GitManager {
    pub fn new() -> Self {
        Self {
            repos: HashMap::new(),
            blames: HashMap::new(),
            statuses: HashMap::new(),
            branches: HashMap::new(),
        }
    }

    pub fn discover_repo(&mut self, file_path: &Path) -> Option<PathBuf> {
        let file_path = if file_path.is_relative() {
            std::env::current_dir().ok()?.join(file_path)
        } else {
            file_path.to_path_buf()
        };
        let mut dir = file_path.parent()?.to_path_buf();
        loop {
            let git_dir = dir.join(".git");
            if git_dir.exists() {
                let repo_path = dir.clone();
                if !self.repos.contains_key(&repo_path) {
                    if let Ok(repo) = git2::Repository::open(&repo_path) {
                        let branch = repo.head().ok()
                            .and_then(|h| h.shorthand().map(|s| s.to_string()))
                            .unwrap_or_else(|| "detached".to_string());
                        self.repos.insert(repo_path.clone(), repo);
                        self.branches.insert(repo_path.clone(), branch);
                    }
                }
                return Some(repo_path);
            }
            if !dir.pop() {
                break;
            }
        }
        None
    }

    pub fn repo_for(&mut self, file_path: &Path) -> Option<&git2::Repository> {
        let repo_path = self.discover_repo(file_path)?;
        self.repos.get(&repo_path)
    }

    pub fn branch_for(&mut self, file_path: &Path) -> Option<String> {
        let repo_path = self.discover_repo(file_path)?;
        self.branches.get(&repo_path).cloned()
    }

    pub fn get_blame(&mut self, file_path: &Path) -> Option<&Vec<BlameInfo>> {
        let path = if file_path.is_relative() {
            std::env::current_dir().ok()?.join(file_path)
        } else {
            file_path.to_path_buf()
        };
        if self.blames.contains_key(&path) {
            return self.blames.get(&path);
        }
        let repo = self.repo_for(&path)?;
        match fetch_blame(repo, &path) {
            Ok(info) => {
                self.blames.insert(path.clone(), info);
                self.blames.get(&path)
            }
            Err(_) => None,
        }
    }

    pub fn clear_blame(&mut self, file_path: &Path) {
        let path = if file_path.is_relative() {
            std::env::current_dir().ok().map(|cwd| cwd.join(file_path))
        } else {
            Some(file_path.to_path_buf())
        };
        if let Some(p) = path {
            self.blames.remove(&p);
        }
    }

    pub fn get_status(&mut self, file_path: &Path) -> Option<&RepoStatus> {
        let repo_path = self.discover_repo(file_path)?;
        let status = fetch_status(self.repos.get(&repo_path)?);
        self.statuses.insert(repo_path.clone(), status);
        self.statuses.get(&repo_path)
    }

    pub fn stage_file(&mut self, file_path: &Path, repo_relative: &str) -> Result<(), String> {
        let repo = self.repo_for(file_path).ok_or("No git repo")?;
        stage_file(repo, repo_relative).map_err(|e| e.to_string())?;
        self.statuses.clear();
        Ok(())
    }

    pub fn unstage_file(&mut self, file_path: &Path, repo_relative: &str) -> Result<(), String> {
        let repo = self.repo_for(file_path).ok_or("No git repo")?;
        unstage_file(repo, repo_relative).map_err(|e| e.to_string())?;
        self.statuses.clear();
        Ok(())
    }
}
