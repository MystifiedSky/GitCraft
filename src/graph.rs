//! Commit graph lane layout, computed in Rust and drawn by the front end on a canvas.
//!
//! Each row gets the column of its node plus line segments for the top half
//! (row top -> node centre) and bottom half (node centre -> row bottom).
//! Colours are the lane index modulo the 10 graph colours of the theme.

use crate::git::Commit;
use serde::Serialize;

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

pub fn layout(commits: &[Commit]) -> GraphLayout {
    // lanes[i] = sha this lane is waiting for (the next commit to arrive in it)
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut out = GraphLayout::default();

    for c in commits {
        // Which lanes are waiting for this commit?
        let waiting: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| l.as_deref() == Some(c.sha.as_str()))
            .map(|(i, _)| i)
            .collect();
        let col = match waiting.first() {
            Some(&i) => i,
            None => alloc(&mut lanes),
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
                let j = alloc(&mut lanes);
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
        while matches!(lanes.last(), Some(None)) {
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
    out
}

fn alloc(lanes: &mut Vec<Option<String>>) -> usize {
    if let Some(i) = lanes.iter().position(|l| l.is_none()) {
        i
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
}
