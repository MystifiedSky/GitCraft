//! Commit graph lane layout, computed in Rust and drawn by the front end on a canvas.
//!
//! Each row gets the column of its node plus line segments for the top half
//! (row top -> node centre) and bottom half (node centre -> row bottom).
//! Colours are the lane index modulo the 10 graph colours of the theme.

use crate::git::Commit;
use serde::{Deserialize, Serialize};

pub const GRAPH_COLORS: usize = 10;

#[derive(Clone, Debug, Serialize)]
pub struct Seg {
    pub from: u16,
    pub to: u16,
    pub color: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphRow {
    pub col: u16,
    pub color: u8,
    pub up: Vec<Seg>,
    pub down: Vec<Seg>,
    pub merge: bool,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct GraphLayout {
    pub rows: Vec<GraphRow>,
    pub width: u16, // most lanes used at once
}

#[allow(dead_code)] // the simple form, used by tests
pub fn layout(commits: &[Commit]) -> GraphLayout {
    layout_pinned(commits, None)
}

/// Layout with an optional pinned branch ("Pin to Left"): the pinned branch's first-parent line
/// always uses lane 0, and every other line starts from lane 1.
pub fn layout_pinned(commits: &[Commit], pinned: Option<&str>) -> GraphLayout {
    let pinned = pinned.filter(|tip| commits.iter().any(|c| c.sha == *tip));
    let mut cursor = LayoutCursor::new(pinned);
    cursor.append(commits)
}

/// Only the active lanes are carried between pages; previous rows never need rebuilding.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LayoutCursor {
    lanes: Vec<Option<String>>,
    pinned_next: Option<String>,
    pinned: bool,
    width: u16,
}

impl LayoutCursor {
    pub fn new(pinned: Option<&str>) -> Self {
        Self { lanes: if pinned.is_some() { vec![None] } else { vec![] }, pinned_next: pinned.map(str::to_owned), pinned: pinned.is_some(), width: 0 }
    }

    pub fn append(&mut self, commits: &[Commit]) -> GraphLayout {
        let lanes = &mut self.lanes;
        let first_free = if self.pinned { 1 } else { 0 };
        let mut out = GraphLayout { rows: Vec::with_capacity(commits.len()), width: self.width };
        for c in commits {
            // Which lanes are waiting for this commit?
            let waiting: Vec<usize> = lanes
                .iter()
                .enumerate()
                .filter(|(_, l)| l.as_deref() == Some(c.sha.as_str()))
                .map(|(i, _)| i)
                .collect();
            let col = if self.pinned_next.as_deref() == Some(c.sha.as_str()) {
                self.pinned_next = c.parents.first().cloned();
                0
            } else {
                match waiting.first() {
                    Some(&i) => i,
                    None => alloc(lanes, first_free),
                }
            };

            let mut up = Vec::new();
            for (i, l) in lanes.iter().enumerate() {
                if l.is_none() {
                    continue;
                }
                if waiting.contains(&i) {
                    up.push(Seg { from: i as u16, to: col as u16, color: (i % GRAPH_COLORS) as u8 });
                } else {
                    up.push(Seg { from: i as u16, to: i as u16, color: (i % GRAPH_COLORS) as u8 });
                }
            }
            for &i in &waiting {
                lanes[i] = None;
            }

            // Continue with the parents.
            let mut down = Vec::new();
            // A lane further right that waits for our first parent is folded into our lane.
            let mut redirect: Option<usize> = None;
            for (pi, p) in c.parents.iter().enumerate() {
                if pi == 0 {
                    // First parent: same lane, unless another lane already waits for it.
                    match lanes.iter().position(|l| l.as_deref() == Some(p.as_str())) {
                        Some(j) if j < col => {
                            down.push(Seg { from: col as u16, to: j as u16, color: (col % GRAPH_COLORS) as u8 });
                        }
                        Some(j) => {
                            lanes[j] = None;
                            lanes[col] = Some(p.clone());
                            redirect = Some(j);
                            down.push(Seg { from: col as u16, to: col as u16, color: (col % GRAPH_COLORS) as u8 });
                        }
                        None => {
                            lanes[col] = Some(p.clone());
                            down.push(Seg { from: col as u16, to: col as u16, color: (col % GRAPH_COLORS) as u8 });
                        }
                    }
                } else if let Some(j) = lanes.iter().position(|l| l.as_deref() == Some(p.as_str())) {
                    down.push(Seg { from: col as u16, to: j as u16, color: (j % GRAPH_COLORS) as u8 });
                } else {
                    let j = alloc(lanes, first_free);
                    lanes[j] = Some(p.clone());
                    down.push(Seg { from: col as u16, to: j as u16, color: (j % GRAPH_COLORS) as u8 });
                }
            }
            // Lanes passing straight through this row.
            for (i, l) in lanes.iter().enumerate() {
                if l.is_some() && i != col && !down.iter().any(|s| s.to as usize == i && s.from as usize == col) {
                    down.push(Seg { from: i as u16, to: i as u16, color: (i % GRAPH_COLORS) as u8 });
                }
            }
            if let Some(j) = redirect {
                down.push(Seg { from: j as u16, to: col as u16, color: (j % GRAPH_COLORS) as u8 });
            }
            // Drop trailing empty lanes so the graph doesn't keep growing.
            while lanes.len() > first_free && matches!(lanes.last(), Some(None)) {
                lanes.pop();
            }

            let used = up.iter().chain(down.iter()).map(|s| s.from.max(s.to) + 1).max().unwrap_or(1).max(col as u16 + 1);
            out.width = out.width.max(used);
            out.rows.push(GraphRow {
                col: col as u16,
                color: (col % GRAPH_COLORS) as u8,
                up,
                down,
                merge: c.parents.len() > 1,
            });
        }
        self.width = out.width;
        out
    }
}

fn alloc(lanes: &mut Vec<Option<String>>, from: usize) -> usize {
    while lanes.len() < from {
        lanes.push(None);
    }
    if let Some(i) = lanes.iter().skip(from).position(|l| l.is_none()) {
        i + from
    } else {
        lanes.push(None);
        lanes.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(sha: &str, parents: &[&str]) -> Commit {
        Commit {
            sha: sha.into(),
            parents: parents.iter().map(|s| s.to_string()).collect(),
            author: String::new(),
            email: String::new(),
            time: 0,
            summary: String::new(),
            body: String::new(),
        }
    }

    #[test]
    fn paged_layout_matches_whole_history_across_merges_and_pinned_branches() {
        let commits = vec![c("m", &["a", "b"]), c("b", &["r"]), c("a", &["r"]), c("r", &[])];
        for pinned in [None, Some("b"), Some("a")] {
            let whole = layout_pinned(&commits, pinned);
            for split in 1..commits.len() {
                let mut cursor = LayoutCursor::new(pinned);
                let mut first = cursor.append(&commits[..split]);
                let encoded = serde_json::to_string(&cursor).unwrap();
                let mut restored: LayoutCursor = serde_json::from_str(&encoded).unwrap();
                let second = restored.append(&commits[split..]);
                first.rows.extend(second.rows);
                first.width = second.width;
                assert_eq!(serde_json::to_value(first).unwrap(), serde_json::to_value(&whole).unwrap());
            }
        }
    }

    #[test]
    fn merge_uses_two_lanes() {
        // m merges b into a; both come from root r
        let commits = vec![c("m", &["a", "b"]), c("b", &["r"]), c("a", &["r"]), c("r", &[])];
        let g = layout(&commits);
        assert_eq!(g.rows[0].col, 0);
        assert_eq!(g.rows[1].col, 1);
        assert_eq!(g.rows[2].col, 0);
        assert_eq!(g.rows[3].col, 0);
        assert_eq!(g.width, 2);
        assert!(g.rows[0].merge);
    }

    #[test]
    fn pinned_branch_stays_in_lane_zero() {
        // m merges b into a; pinning b moves b's line to lane 0 and a's to lane 1.
        let commits = vec![c("m", &["a", "b"]), c("b", &["r"]), c("a", &["r"]), c("r", &[])];
        let g = layout_pinned(&commits, Some("b"));
        assert_eq!(g.rows[1].col, 0, "b is pinned");
        assert_eq!(g.rows[3].col, 0, "its parent r too");
        assert_eq!(g.rows[0].col, 1);
        assert_eq!(g.rows[2].col, 1);
    }

}
