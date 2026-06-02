use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::time::Instant;
use crate::git::blame::{fetch_blame, fetch_blame_range, BlameInfo};
use crate::git::status::{fetch_status, RepoStatus};
use crate::git::operations::{stage_file, unstage_file};

const CACHE_TTL_SECS: u64 = 60;
const DEFAULT_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(CACHE_TTL_SECS);

pub struct GitManager {
    root: PathBuf,
    repos: HashMap<PathBuf, (git2::Repository, Instant)>,
    blames: HashMap<PathBuf, (Vec<BlameInfo>, Instant)>,
    statuses: HashMap<PathBuf, (RepoStatus, Instant)>,
    branches: HashMap<PathBuf, (String, Instant)>,
}

impl GitManager {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new_with_root(root)
    }

    pub fn new_with_root(root: PathBuf) -> Self {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        Self {
            root,
            repos: HashMap::new(),
            blames: HashMap::new(),
            statuses: HashMap::new(),
            branches: HashMap::new(),
        }
    }

    fn is_fresh(ts: Instant) -> bool {
        ts.elapsed() < DEFAULT_CACHE_TTL
    }

    fn cache_key(file_path: &Path, root: &Path) -> PathBuf {
        let abs = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            root.join(file_path)
        };
        std::fs::canonicalize(&abs).unwrap_or(abs)
    }

    pub fn root_path(&self) -> &Path {
        &self.root
    }

    pub fn discover_repo(&mut self, file_path: &Path) -> Option<PathBuf> {
        let abs = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            self.root.join(file_path)
        };
        let canon = std::fs::canonicalize(&abs).unwrap_or(abs);

        let discovered = git2::Repository::discover(&canon).ok()?;
        let workdir = discovered.workdir()?.to_path_buf();
        let workdir_canon = std::fs::canonicalize(&workdir).unwrap_or(workdir.clone());
        let repo_path = workdir_canon.clone();

        let is_cached = matches!(self.repos.get(&repo_path), Some((_, ts)) if Self::is_fresh(*ts));
        if is_cached {
            return Some(repo_path);
        }

        if let Ok(mut cfg) = discovered.config() {
            let _ = cfg.set_str("core.fsync", "objects,index,reference");
        }

        let branch = discovered.head().ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "detached".to_string());
        self.repos.insert(repo_path.clone(), (discovered, Instant::now()));
        self.branches.insert(repo_path.clone(), (branch, Instant::now()));
        Some(repo_path)
    }

    pub fn repo_for(&mut self, file_path: &Path) -> Option<&git2::Repository> {
        let repo_path = self.discover_repo(file_path)?;
        self.repos.get(&repo_path).map(|(r, _)| r)
    }

    pub fn branch_for(&mut self, file_path: &Path) -> Option<String> {
        let repo_path = self.discover_repo(file_path)?;
        self.branches.get(&repo_path).map(|(b, _)| b.clone())
    }

    pub fn get_blame(&mut self, file_path: &Path) -> Option<&Vec<BlameInfo>> {
        let key = Self::cache_key(file_path, &self.root);
        let fresh = self.blames.get(&key)
            .map(|(_, ts)| Self::is_fresh(*ts))
            .unwrap_or(false);
        if fresh {
            return self.blames.get(&key).map(|(v, _)| v);
        }
        self.blames.remove(&key);
        let repo = self.repo_for(&key)?;
        match fetch_blame(repo, &key) {
            Ok(info) => {
                self.blames.insert(key.clone(), (info, Instant::now()));
            }
            Err(_) => return None,
        }
        self.blames.get(&key).map(|(v, _)| v)
    }

    pub fn get_blame_range(&mut self, file_path: &Path, range: Option<(usize, usize)>) -> Option<&Vec<BlameInfo>> {
        let key = Self::cache_key(file_path, &self.root);
        let fresh = self.blames.get(&key)
            .map(|(_, ts)| Self::is_fresh(*ts))
            .unwrap_or(false);
        if fresh {
            return self.blames.get(&key).map(|(v, _)| v);
        }
        self.blames.remove(&key);
        let repo = self.repo_for(&key)?;
        match fetch_blame_range(repo, &key, range) {
            Ok(info) => {
                self.blames.insert(key.clone(), (info, Instant::now()));
            }
            Err(_) => return None,
        }
        self.blames.get(&key).map(|(v, _)| v)
    }

    pub fn clear_blame(&mut self, file_path: &Path) {
        let key = Self::cache_key(file_path, &self.root);
        self.blames.remove(&key);
    }

    pub fn get_status(&mut self, file_path: &Path) -> Option<&RepoStatus> {
        let repo_path = self.discover_repo(file_path)?;
        let fresh = self.statuses.get(&repo_path)
            .map(|(_, ts)| Self::is_fresh(*ts))
            .unwrap_or(false);
        if fresh {
            return self.statuses.get(&repo_path).map(|(v, _)| v);
        }
        let repo_entry = self.repos.get(&repo_path)?;
        let status = fetch_status(&repo_entry.0);
        self.statuses.insert(repo_path.clone(), (status, Instant::now()));
        self.statuses.get(&repo_path).map(|(v, _)| v)
    }

    pub fn stage_file(&mut self, file_path: &Path, repo_relative: &str) -> Result<(), String> {
        let repo = self.repo_for(file_path).ok_or("No git repo")?;
        stage_file(repo, repo_relative).map_err(|e| e.to_string())?;
        self.invalidate_statuses();
        Ok(())
    }

    pub fn unstage_file(&mut self, file_path: &Path, repo_relative: &str) -> Result<(), String> {
        let repo = self.repo_for(file_path).ok_or("No git repo")?;
        unstage_file(repo, repo_relative).map_err(|e| e.to_string())?;
        self.invalidate_statuses();
        Ok(())
    }

    fn perform_checkout(
        repos: &mut HashMap<PathBuf, (git2::Repository, Instant)>,
        repo_path: &Path,
        branch: &str,
    ) -> Result<(), String> {
        let key = repo_path.to_path_buf();
        let branch_obj = {
            let entry = repos.get(&key).ok_or("Repo not found")?;
            entry.0.find_branch(branch, git2::BranchType::Local)
                .map_err(|e| format!("invalid branch '{}': {}", branch, e))?
        };
        let refname = branch_obj.get().name()
            .ok_or_else(|| format!("invalid ref name for branch '{}'", branch))?
            .to_string();
        let object = {
            let entry = repos.get(&key).ok_or("Repo not found")?;
            let (object, reference) = entry.0.revparse_ext(&refname)
                .map_err(|e| format!("resolve branch: {}", e))?;
            let _reference = reference.ok_or_else(|| format!("'{}' is not a branch", branch))?;
            object
        };
        {
            let entry = repos.get(&key).ok_or("Repo not found")?;
            entry.0.checkout_tree(&object, None)
                .map_err(|e| format!("checkout tree: {}", e))?;
            entry.0.set_head(&refname)
                .map_err(|e| format!("set head: {}", e))?;
        }
        Ok(())
    }

    pub fn checkout_branch(&mut self, file_path: &Path, branch: &str) -> Result<String, String> {
        let repo_path = self.discover_repo(file_path).ok_or("No git repo")?;
        Self::perform_checkout(&mut self.repos, &repo_path, branch)?;
        self.branches.insert(repo_path, (branch.to_string(), Instant::now()));
        self.invalidate_all_caches();
        Ok(format!("Switched to branch '{}'", branch))
    }

    pub fn invalidate(&mut self, repo_path: &Path) {
        let canon = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());
        self.repos.remove(&canon);
        self.branches.remove(&canon);
        self.statuses.remove(&canon);
        self.blames.retain(|k, _| {
            let parent = k.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            !parent.starts_with(&canon)
        });
    }

    pub fn invalidate_all(&mut self) {
        self.invalidate_all_caches();
    }

    fn invalidate_statuses(&mut self) {
        self.statuses.clear();
    }

    fn invalidate_all_caches(&mut self) {
        self.repos.clear();
        self.branches.clear();
        self.blames.clear();
        self.statuses.clear();
    }
}
