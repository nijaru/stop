//! `stop tree` — renders parent/child relationships from a snapshot.
//!
//! Output is a spanning forest over the collected snapshot: every process
//! appears exactly once. Roots are processes whose parent is absent
//! (exited or ppid unavailable). Cycles from PID reuse are broken at the
//! first back-edge; processes trapped in a cycle with no reachable root
//! are emitted as additional roots so nothing is silently dropped. With a
//! target, the forest is the subtree rooted at that process.

use std::collections::HashMap;

use serde::Serialize;

use crate::cli::{CollectionArgs, OutputArgs, TreeArgs};
use crate::cmd::{Outcome, resolve};
use crate::collector;
use crate::error::StopError;
use crate::model::{CollectionMetadata, ProcessInfo};
use crate::output;
use crate::runtime::Cancellation;

#[derive(Serialize)]
pub struct TreeNode {
    pub process: ProcessInfo,
    pub children: Vec<TreeNode>,
}

#[derive(Serialize)]
pub struct TreeReport {
    #[serde(flatten)]
    pub collection: CollectionMetadata,
    pub total_processes: usize,
    pub roots: Vec<TreeNode>,
}

pub fn run(
    args: &TreeArgs,
    collection: &CollectionArgs,
    output_args: &OutputArgs,
    cancel: &Cancellation,
) -> Result<Outcome, StopError> {
    let data = collector::collect(
        collector::Request {
            cpu: collection.cpu.is_some(),
            ..collector::Request::default()
        },
        collection.cpu,
        cancel,
    )?;
    // A forest covers every successfully collected row, not PIDs lost during acquisition.
    let total_processes = data.processes.len();
    let procs = data.processes;
    let target = args.name.as_deref().or(args.target.as_deref());
    let roots = match target {
        Some(target) => match resolve::resolve(&procs, target, args.name.is_some()) {
            Ok(index) => build_subtree(procs, index),
            Err(err) if err.code == "not_found" => vec![],
            Err(err) => return Err(err.into()),
        },
        None => build_forest(procs),
    };
    let report = TreeReport {
        collection: data.metadata,
        total_processes,
        roots,
    };
    if output_args.document() {
        output::print_json(&report, output_args.pretty, cancel)?;
    } else if report.roots.is_empty() {
        output::print_no_match(target.unwrap_or("any process"), cancel)?;
    } else {
        output::print_process_tree(&report.roots, collection.cpu.is_some(), cancel)?;
    }
    Ok(if report.roots.is_empty() {
        Outcome::NoMatch
    } else {
        Outcome::Success
    })
}

/// Sorted snapshot plus adjacency (parent index -> child indices) and the
/// root indices. Children inherit PID order.
fn prepare(
    mut procs: Vec<ProcessInfo>,
) -> (Vec<ProcessInfo>, HashMap<usize, Vec<usize>>, Vec<usize>) {
    procs.sort_by_key(|p| p.pid);
    let index: HashMap<u32, usize> = procs.iter().enumerate().map(|(i, p)| (p.pid, i)).collect();

    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, p) in procs.iter().enumerate() {
        if let Some(pp) = p.ppid.and_then(|pp| index.get(&pp).copied())
            && pp != i
            && !matches!((procs[pp].start_time, p.start_time), (Some(parent), Some(child)) if parent > child)
        {
            adj.entry(pp).or_default().push(i);
        }
    }

    let roots: Vec<usize> = procs
        .iter()
        .enumerate()
        .filter(|(_, p)| p.ppid.is_none_or(|pp| !index.contains_key(&pp)))
        .map(|(i, _)| i)
        .collect();

    (procs, adj, roots)
}

/// Depth-first walk from `root`, skipping already-visited nodes so cycles
/// break at the first back-edge.
fn walk(
    root: usize,
    procs: &[ProcessInfo],
    adj: &HashMap<usize, Vec<usize>>,
    visited: &mut [bool],
) -> TreeNode {
    visited[root] = true;
    let mut children = Vec::new();
    if let Some(child_indices) = adj.get(&root) {
        for &c in child_indices {
            if !visited[c] {
                children.push(walk(c, procs, adj, visited));
            }
        }
    }
    TreeNode {
        process: procs[root].clone(),
        children,
    }
}

/// Spanning forest over the snapshot. Invariant: every process appears
/// exactly once.
fn build_forest(procs: Vec<ProcessInfo>) -> Vec<TreeNode> {
    let (procs, adj, roots) = prepare(procs);
    let n = procs.len();
    let mut visited = vec![false; n];
    let mut forest: Vec<TreeNode> = Vec::new();

    for r in roots {
        if !visited[r] {
            forest.push(walk(r, &procs, &adj, &mut visited));
        }
    }
    // Cycles with no reachable root: re-root them so no process is dropped.
    for i in 0..n {
        if !visited[i] {
            forest.push(walk(i, &procs, &adj, &mut visited));
        }
    }
    forest
}

/// Subtree rooted at `root` (an index into `procs`).
fn build_subtree(procs: Vec<ProcessInfo>, root: usize) -> Vec<TreeNode> {
    let target_pid = procs[root].pid;
    let (procs, adj, _roots) = prepare(procs);
    let idx = procs
        .iter()
        .position(|p| p.pid == target_pid)
        .expect("root was taken from procs");
    let mut visited = vec![false; procs.len()];
    vec![walk(idx, &procs, &adj, &mut visited)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, ppid: Option<u32>, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            start_time: Some(1_700_000_000),
            birth_id: Some(format!("test:{pid}")),
            unavailable: Default::default(),
            ppid,
            name: name.to_string(),
            exe: None,
            cmdline: vec![],
            cwd: None,
            state: "run".to_string(),
            user: None,
            uid: None,
            cpu_percent: None,
            rss_bytes: Some(0),
            virtual_bytes: Some(0),
            threads: None,
            io_read_bytes: Some(0),
            io_written_bytes: Some(0),
        }
    }

    fn count_nodes(roots: &[TreeNode]) -> usize {
        roots.iter().map(|r| 1 + count_nodes(&r.children)).sum()
    }

    fn pids(roots: &[TreeNode]) -> Vec<u32> {
        roots.iter().map(|r| r.process.pid).collect()
    }

    #[test]
    fn forest_covers_every_process_exactly_once() {
        let procs = vec![
            proc(1, None, "a"),
            proc(2, Some(1), "b"),
            proc(3, Some(2), "c"),
            proc(4, Some(1), "d"),
        ];
        let forest = build_forest(procs);
        assert_eq!(forest.len(), 1);
        assert_eq!(count_nodes(&forest), 4);

        let root = &forest[0];
        assert_eq!(root.process.pid, 1);
        assert_eq!(pids(&root.children), vec![2, 4], "children in PID order");
        assert_eq!(pids(&root.children[0].children), vec![3]);
    }

    #[test]
    fn absent_parent_becomes_root() {
        let procs = vec![proc(20, None, "root"), proc(10, Some(99), "orphan")];
        let forest = build_forest(procs);
        assert_eq!(pids(&forest), vec![10, 20], "roots in PID order");
    }

    #[test]
    fn two_cycle_is_broken_without_dropping_nodes() {
        // 5 -> 6 -> 5 (PID reuse cycle), plus a normal chain 1 -> 2.
        let procs = vec![
            proc(1, None, "a"),
            proc(2, Some(1), "b"),
            proc(5, Some(6), "c"),
            proc(6, Some(5), "d"),
        ];
        let forest = build_forest(procs);
        assert_eq!(count_nodes(&forest), 4, "no process dropped by the cycle");
        assert_eq!(pids(&forest), vec![1, 5], "cycle members re-rooted");

        let c = &forest[1];
        assert_eq!(c.process.pid, 5);
        assert_eq!(pids(&c.children), vec![6]);
        assert!(
            c.children[0].children.is_empty(),
            "back-edge to 5 is dropped"
        );
    }

    #[test]
    fn self_parent_is_a_single_node() {
        let forest = build_forest(vec![proc(7, Some(7), "loop")]);
        assert_eq!(count_nodes(&forest), 1);
        assert!(forest[0].children.is_empty());
    }

    #[test]
    fn subtree_contains_only_descendants() {
        let procs = vec![
            proc(1, None, "a"),
            proc(2, Some(1), "b"),
            proc(3, Some(2), "c"),
            proc(4, None, "other"),
        ];
        let forest = build_subtree(procs, 1);
        assert_eq!(forest.len(), 1);
        assert_eq!(forest[0].process.pid, 2);
        assert_eq!(count_nodes(&forest), 2);
        assert_eq!(pids(&forest[0].children), vec![3]);
    }

    #[test]
    fn subtree_breaks_descendant_cycles() {
        // 2 <-> 3 are mutually parented (PID reuse); rooting at 2 must not loop.
        let procs = vec![proc(2, Some(3), "b"), proc(3, Some(2), "c")];
        let forest = build_subtree(procs, 0);
        assert_eq!(forest.len(), 1);
        assert_eq!(forest[0].process.pid, 2);
        assert_eq!(count_nodes(&forest), 2);
        assert!(forest[0].children[0].children.is_empty());
    }
}
