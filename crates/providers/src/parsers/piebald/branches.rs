// crates/providers/src/parsers/piebald/branches.rs
//
// Piebald messages form a DAG via parent_message_id (edits/retries create
// sibling children). Port of agentsview's splitPiebaldBranches: with a
// single root, the MAIN path follows the last ENABLED child at every branch
// point; every bypassed child spawns a fork branch (recursively, so nested
// forks all surface). Multiple roots (or orphaned parents) → one flat
// branch in row order.

use super::db::MsgRow;
use std::collections::HashMap;

pub(super) struct Branch {
    /// Indices into the chat's message rows, in path order.
    pub(super) rows: Vec<usize>,
    /// `Some(first message row id)` for fork branches (drives the
    /// `<chat>-<row>` raw session id); `None` for the main/flat branch.
    pub(super) first_row_id: Option<i64>,
}

pub(super) fn split_branches(msgs: &[MsgRow]) -> Vec<Branch> {
    let mut by_id: HashMap<i64, usize> = HashMap::with_capacity(msgs.len());
    for (i, m) in msgs.iter().enumerate() {
        by_id.insert(m.id, i);
    }
    let mut children: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();
    for (i, m) in msgs.iter().enumerate() {
        // A row whose parent is missing from the chat counts as a root.
        match m.parent_message_id.filter(|p| by_id.contains_key(p)) {
            Some(parent) => children.entry(parent).or_default().push(i),
            None => roots.push(i),
        }
    }
    if roots.len() != 1 {
        // No single root → don't guess a tree; emit the rows flat, in order.
        return vec![Branch {
            rows: (0..msgs.len()).collect(),
            first_row_id: None,
        }];
    }
    let mut forks: Vec<Branch> = Vec::new();
    let main = walk_main_path(msgs, &children, roots[0], &mut forks);
    let mut out = Vec::with_capacity(1 + forks.len());
    out.push(Branch {
        rows: main,
        first_row_id: None,
    });
    out.append(&mut forks);
    out
}

/// Follow the main path from `start`, spawning a fork branch for every
/// non-main child at each branch point. The recursive call runs BEFORE the
/// fork is appended, so nested forks land in `forks` ahead of their outer
/// fork — mirroring the evaluation-order fix in the Go source.
fn walk_main_path(
    msgs: &[MsgRow],
    children: &HashMap<i64, Vec<usize>>,
    start: usize,
    forks: &mut Vec<Branch>,
) -> Vec<usize> {
    let mut path = vec![start];
    let mut current = start;
    while let Some(kids) = children.get(&msgs[current].id) {
        if path.len() > msgs.len() {
            break; // cycle guard: a valid path can never exceed the row count
        }
        // Main continuation = last enabled child; if none is enabled, the
        // last child (Go parity).
        let main_idx = kids
            .iter()
            .rposition(|&k| msgs[k].enabled)
            .unwrap_or(kids.len() - 1);
        for (i, &kid) in kids.iter().enumerate() {
            if i != main_idx {
                let rows = walk_main_path(msgs, children, kid, forks);
                forks.push(Branch {
                    rows,
                    first_row_id: Some(msgs[kid].id),
                });
            }
        }
        current = kids[main_idx];
        path.push(current);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: i64, parent: Option<i64>, enabled: bool) -> MsgRow {
        MsgRow {
            id,
            parent_message_id: parent,
            enabled,
            role: "user".into(),
            model: String::new(),
            created_at: String::new(),
            input_tokens: None,
            output_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }

    #[test]
    fn multiple_roots_fall_back_to_flat() {
        let msgs = vec![
            msg(1, None, true),
            msg(2, None, true),
            msg(3, Some(2), true),
        ];
        let b = split_branches(&msgs);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].rows, vec![0, 1, 2]);
        assert_eq!(b[0].first_row_id, None);
    }

    #[test]
    fn branch_point_follows_last_enabled_child_and_forks_the_rest() {
        // 1 → {2 (disabled), 3 (enabled), 4 (disabled)}; main path = 1 → 3.
        let msgs = vec![
            msg(1, None, true),
            msg(2, Some(1), false),
            msg(3, Some(1), true),
            msg(4, Some(1), false),
        ];
        let b = split_branches(&msgs);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].rows, vec![0, 2]);
        assert_eq!(b[0].first_row_id, None);
        let fork_ids: Vec<_> = b[1..].iter().map(|f| f.first_row_id).collect();
        assert_eq!(fork_ids, vec![Some(2), Some(4)]);
    }
}
