use git2::{Oid, Repository, BranchType};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct CommitData {
    pub oid: Oid,
    pub short_oid: String,
    pub subject: String,
    pub parents: Vec<Oid>,
    pub children: Vec<Oid>,
    pub refs: Vec<(String, bool)>,
    pub timestamp: i64,
    pub lane: usize,
    pub merge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphChar {
    Commit,        // ●
    MergeCommit,   // ○
    Vertical,      // │
    BranchRight,   // ├
    MergeLeft,     // ┤
    BottomRight,   // └
    BottomLeft,    // ┘
    Horizontal,    // ─
    DiagonalLeft,  // ╱
    DiagonalRight, // ╲
    Empty,
}

#[derive(Debug, Clone)]
pub struct DisplayRow {
    pub text: String,
    pub branch: Option<String>,
    pub is_head: bool,
}

pub struct GraphRenderer {
    pub rows: Vec<DisplayRow>,
}

impl GraphRenderer {
    pub fn new(repo: &Repository) -> Self {
        let commits = Self::collect_commits(repo);
        let rows = Self::build_display(&commits);
        GraphRenderer { rows }
    }

    fn collect_commits(repo: &Repository) -> Vec<CommitData> {
        let mut refs: Vec<(String, Oid, bool)> = Vec::new();
        let mut ref_oids: HashSet<Oid> = HashSet::new();

        if let Ok(head) = repo.head() {
            if let Some(oid) = head.target() {
                let name = head.shorthand().unwrap_or("HEAD").to_string();
                if ref_oids.insert(oid) {
                    refs.push((name, oid, true));
                }
            }
        }

        if let Ok(branches) = repo.branches(Some(BranchType::Local)) {
            for branch in branches.flatten() {
                let name = branch.0.name().ok().flatten().unwrap_or("").to_string();
                if let Some(oid) = branch.0.get().target() {
                    if ref_oids.insert(oid) {
                        refs.push((name, oid, false));
                    }
                }
            }
        }

        let mut commit_map: HashMap<Oid, CommitData> = HashMap::new();
        let mut queue: VecDeque<Oid> = refs.iter().map(|(_, o, _)| *o).collect();
        let mut visited: HashSet<Oid> = HashSet::new();

        while let Some(oid) = queue.pop_front() {
            if !visited.insert(oid) {
                continue;
            }
            if let Ok(commit) = repo.find_commit(oid) {
                let msg = commit.message().unwrap_or("").to_string();
                let subject = msg.lines().next().unwrap_or("").to_string();
                let parents: Vec<Oid> = commit.parents().map(|p| p.id()).collect();
                let oid_str = oid.to_string();
                let short = oid_str[..7.min(oid_str.len())].to_string();
                commit_map.insert(oid, CommitData {
                    oid,
                    short_oid: short,
                    subject,
                    parents: parents.clone(),
                    children: Vec::new(),
                    refs: Vec::new(),
                    timestamp: commit.time().seconds(),
                    lane: 0,
                    merge: parents.len() > 1,
                });
                for p in &parents {
                    queue.push_back(*p);
                }
            }
        }

        let oids: Vec<Oid> = commit_map.keys().cloned().collect();
        for oid in &oids {
            let parents = commit_map.get(oid).map(|c| c.parents.clone()).unwrap_or_default();
            for p in &parents {
                if let Some(child) = commit_map.get_mut(p) {
                    child.children.push(*oid);
                }
            }
        }

        for (name, oid, is_head) in &refs {
            if let Some(commit) = commit_map.get_mut(oid) {
                commit.refs.push((name.clone(), *is_head));
            }
        }

        // Topological sort: Kahn's algorithm
        let mut in_degree: HashMap<Oid, usize> = HashMap::new();
        let mut rev_graph: HashMap<Oid, Vec<Oid>> = HashMap::new();

        for (oid, data) in &commit_map {
            in_degree.entry(*oid).or_insert(0);
            for p in &data.parents {
                if commit_map.contains_key(p) {
                    rev_graph.entry(*p).or_default().push(*oid);
                    *in_degree.entry(*oid).or_insert(0) += 1;
                }
            }
        }

        let mut queue: VecDeque<Oid> = VecDeque::new();
        for (oid, deg) in &in_degree {
            if *deg == 0 {
                queue.push_back(*oid);
            }
        }

        let mut sorted: Vec<CommitData> = Vec::new();
        while !queue.is_empty() {
            queue.make_contiguous().sort_by(|a, b| {
                let ta = commit_map.get(a).map(|c| c.timestamp).unwrap_or(0);
                let tb = commit_map.get(b).map(|c| c.timestamp).unwrap_or(0);
                tb.cmp(&ta)
            });
            let oid = queue.pop_front().unwrap();
            if let Some(data) = commit_map.get(&oid) {
                sorted.push(data.clone());
            }
            if let Some(children) = rev_graph.get(&oid) {
                let mut to_add: Vec<Oid> = children.iter().filter(|c| {
                    if let Some(deg) = in_degree.get_mut(c) {
                        *deg -= 1;
                        *deg == 0
                    } else {
                        false
                    }
                }).cloned().collect();
                to_add.sort_by(|a, b| {
                    let ta = commit_map.get(a).map(|c| c.timestamp).unwrap_or(0);
                    let tb = commit_map.get(b).map(|c| c.timestamp).unwrap_or(0);
                    tb.cmp(&ta)
                });
                for c in to_add {
                    queue.push_back(c);
                }
            }
        }

        sorted.reverse();
        sorted
    }

    fn build_display(commits: &[CommitData]) -> Vec<DisplayRow> {
        if commits.is_empty() {
            return Vec::new();
        }

        let mut columns: Vec<(Oid, Vec<(String, bool)>)> = Vec::new();
        let mut rows: Vec<DisplayRow> = Vec::new();

        for commit_idx in 0..commits.len() {
            let commit = &commits[commit_idx];

            let before_cols: Vec<Oid> = columns.iter().map(|(o, _)| *o).collect();

            let (lane, _) = Self::find_or_create_column(&mut columns, commit.oid);

            let col_branches = &columns[lane].1;
            let branch_name = if !commit.refs.is_empty() {
                commit.refs.iter()
                    .find(|(_, h)| !*h)
                    .or_else(|| commit.refs.first())
                    .map(|(n, _)| n.clone())
            } else {
                col_branches.iter()
                    .find(|(_, h)| !*h)
                    .or_else(|| col_branches.first())
                    .map(|(n, _)| n.clone())
            };
            let row_is_head = commit.refs.iter().any(|(_, h)| *h)
                || col_branches.iter().any(|(_, h)| *h);

            let max_cols = columns.len();
            let mut chars: Vec<GraphChar> = vec![GraphChar::Empty; max_cols];

            if lane < max_cols {
                chars[lane] = if commit.merge { GraphChar::MergeCommit } else { GraphChar::Commit };
            }

            let active_parents: Vec<Oid> = commit.parents.iter()
                .filter(|pp| before_cols.contains(pp) || columns.iter().any(|(c, _)| *c == **pp))
                .cloned()
                .collect();

            for &p_oid in &active_parents {
                if let Some(p_lane) = columns.iter().position(|(c, _)| *c == p_oid) {
                    if p_lane != lane {
                        let (left, right) = if p_lane < lane { (p_lane, lane) } else { (lane, p_lane) };
                        for col in left..=right {
                            if col == p_lane {
                                chars[col] = if p_lane < lane {
                                    GraphChar::MergeLeft
                                } else {
                                    GraphChar::BranchRight
                                };
                            } else if col != lane && chars[col] == GraphChar::Empty {
                                chars[col] = GraphChar::Horizontal;
                            }
                        }
                    }
                }
            }

            for col in 0..max_cols {
                if col == lane { continue; }
                if col < before_cols.len() {
                    let before_oid = before_cols[col];
                    let in_after = columns.iter().any(|(c, _)| *c == before_oid)
                        || active_parents.contains(&before_oid);
                    if in_after && chars[col] == GraphChar::Empty {
                        chars[col] = GraphChar::Vertical;
                    }
                }
            }

            let graph_str = Self::chars_to_string(&chars);

            let refs_str = commit.refs.iter()
                .map(|(n, h)| if *h { format!("*{}", n) } else { n.clone() })
                .collect::<Vec<_>>()
                .join(" ");

            let refs_part = if refs_str.is_empty() {
                String::new()
            } else {
                format!(" ({})", refs_str)
            };
            let text = format!("{} {}{} {}", graph_str, commit.short_oid, refs_part, commit.subject);

            rows.push(DisplayRow {
                text,
                branch: branch_name,
                is_head: row_is_head,
            });

            let after_cols = Self::update_columns(&mut columns, commit, commits, lane);

            if before_cols != after_cols {
                let con_chars = Self::render_connector_row(&before_cols, &after_cols, &columns, commit);
                if con_chars.iter().any(|c| *c != GraphChar::Empty) {
                    let con_str = Self::chars_to_string(&con_chars);
                    let con_row = DisplayRow {
                        text: con_str,
                        branch: None,
                        is_head: false,
                    };
                    rows.push(con_row);
                }
            }
        }

        rows
    }

    fn find_or_create_column(columns: &mut Vec<(Oid, Vec<(String, bool)>)>, oid: Oid) -> (usize, bool) {
        if let Some(pos) = columns.iter().position(|(c, _)| *c == oid) {
            (pos, false)
        } else {
            columns.push((oid, Vec::new()));
            (columns.len() - 1, true)
        }
    }

    fn update_columns(
        columns: &mut Vec<(Oid, Vec<(String, bool)>)>,
        commit: &CommitData,
        all_commits: &[CommitData],
        lane: usize,
    ) -> Vec<Oid> {
        let parents: Vec<Oid> = commit.parents.iter()
            .filter(|p| all_commits.iter().any(|a| a.oid == **p))
            .cloned()
            .collect();

        if parents.is_empty() {
            if lane < columns.len() {
                columns.remove(lane);
            }
        } else {
            let existing_branches = if commit.refs.is_empty() {
                columns[lane].1.clone()
            } else {
                let mut merged = columns[lane].1.clone();
                for (n, h) in &commit.refs {
                    if !merged.iter().any(|(mn, mh)| mn == n && *mh == *h) {
                        merged.push((n.clone(), *h));
                    }
                }
                merged
            };

            columns[lane] = (parents[0], existing_branches);

            for (j, p) in parents[1..].iter().enumerate() {
                if !columns.iter().any(|(c, _)| *c == *p) {
                    let insert_pos = (lane + 1 + j).min(columns.len());
                    columns.insert(insert_pos, (*p, Vec::new()));
                }
            }

            let mut seen = HashSet::new();
            let mut deduped: Vec<(Oid, Vec<(String, bool)>)> = Vec::new();
            for entry in columns.drain(..) {
                if seen.insert(entry.0) {
                    deduped.push(entry);
                }
            }
            *columns = deduped;
        }

        columns.iter().map(|(o, _)| *o).collect()
    }

    fn render_connector_row(
        before: &[Oid],
        after: &[Oid],
        _columns: &[(Oid, Vec<(String, bool)>)],
        _commit: &CommitData,
    ) -> Vec<GraphChar> {
        let max_cols = before.len().max(after.len());
        let mut chars: Vec<GraphChar> = vec![GraphChar::Empty; max_cols];

        for col in 0..max_cols {
            let in_before = col < before.len();
            let in_after = col < after.len();

            if in_before && in_after {
                if before[col] == after[col] {
                    chars[col] = GraphChar::Vertical;
                } else {
                    let old_in_new = after.iter().position(|c| *c == before[col]);
                    if let Some(new_pos) = old_in_new {
                        if new_pos != col {
                            chars[col] = if new_pos < col {
                                GraphChar::DiagonalLeft
                            } else {
                                GraphChar::DiagonalRight
                            };
                        } else {
                            chars[col] = GraphChar::Vertical;
                        }
                    } else {
                        chars[col] = GraphChar::Vertical;
                    }
                }
            } else if in_before && !in_after {
                let old_in_new = after.iter().position(|c| *c == before[col]);
                if let Some(new_pos) = old_in_new {
                    chars[col] = if new_pos < col {
                        GraphChar::DiagonalLeft
                    } else {
                        GraphChar::DiagonalRight
                    };
                } else {
                    chars[col] = GraphChar::BottomLeft;
                }
            } else if !in_before && in_after {
                chars[col] = GraphChar::Empty;
            }
        }

        chars
    }

    fn chars_to_string(chars: &[GraphChar]) -> String {
        let mut s = String::new();
        for ch in chars {
            s.push_str(ch.to_char());
            s.push(' ');
        }
        s.trim_end().to_string()
    }
}

impl GraphChar {
    pub fn to_char(self) -> &'static str {
        match self {
            GraphChar::Commit => "●",
            GraphChar::MergeCommit => "○",
            GraphChar::Vertical => "│",
            GraphChar::BranchRight => "├",
            GraphChar::MergeLeft => "┤",
            GraphChar::BottomRight => "└",
            GraphChar::BottomLeft => "┘",
            GraphChar::Horizontal => "─",
            GraphChar::DiagonalLeft => "╱",
            GraphChar::DiagonalRight => "╲",
            GraphChar::Empty => " ",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;

    #[test]
    fn test_render_linear() {
        let repo_dir = std::env::current_dir().unwrap();
        let repo = Repository::open(&repo_dir).expect("open repo");
        let renderer = GraphRenderer::new(&repo);
        assert!(!renderer.rows.is_empty(), "should have rows");
        for row in &renderer.rows {
            println!("{}", row.text);
        }
    }
}
