// src/semantics/priority.rs
//! Priority graph resolution for PASTA
//!
//! Handles:
//! - DO X OVER Y → directed edge X → Y
//! - Chained overrides: DO Z OVER Y AND Y OVER X
//! - Cycle detection: X > Y > X
//! - Final priority ordering via topological sort
//! - Weight assignment using positional decay (0.75 per step)
//!
//! This module is used by the scheduler to compute thread weights.

use std::collections::{HashMap, HashSet, VecDeque};
use anyhow::{anyhow, Result};

/// Represents a directed priority graph:
/// A → B means "A has higher priority than B".
#[derive(Debug, Clone)]
pub struct PriorityGraph {
    edges: HashMap<String, Vec<String>>,
    nodes: HashSet<String>,
}

impl PriorityGraph {
    /// Create an empty priority graph.
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            nodes: HashSet::new(),
        }
    }

    /// Add a priority override: `higher OVER lower`
    pub fn add_edge(&mut self, higher: &str, lower: &str) {
        self.nodes.insert(higher.to_string());
        self.nodes.insert(lower.to_string());
        self.edges
            .entry(higher.to_string())
            .or_default()
            .push(lower.to_string());
    }

    /// Detect cycles using DFS.
    fn detect_cycle(&self) -> Result<()> {
        fn dfs(
            node: &str,
            edges: &HashMap<String, Vec<String>>,
            visiting: &mut HashSet<String>,
            visited: &mut HashSet<String>,
        ) -> Result<()> {
            if visiting.contains(node) {
                return Err(anyhow!("Priority cycle detected involving '{}'", node));
            }
            if visited.contains(node) {
                return Ok(());
            }

            visiting.insert(node.to_string());

            if let Some(children) = edges.get(node) {
                for child in children {
                    dfs(child, edges, visiting, visited)?;
                }
            }

            visiting.remove(node);
            visited.insert(node.to_string());
            Ok(())
        }

        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        for node in &self.nodes {
            dfs(node, &self.edges, &mut visiting, &mut visited)?;
        }

        Ok(())
    }

    /// Compute final priority order using topological sort.
    ///
    /// Returns a vector of nodes in descending priority order:
    /// highest priority first.
    pub fn resolve_order(&self) -> Result<Vec<String>> {
        self.detect_cycle()?; // fail early if conflict exists

        // Compute in-degree for Kahn's algorithm
        let mut indegree: HashMap<String, usize> =
            self.nodes.iter().map(|n| (n.clone(), 0)).collect();

        for (_src, targets) in &self.edges {
            for t in targets {
                if let Some(entry) = indegree.get_mut(t) {
                    *entry += 1;
                } else {
                    // If target wasn't in nodes for some reason, insert with indegree 1
                    indegree.insert(t.clone(), 1);
                }
            }
        }

        // Queue of nodes with no incoming edges (highest priority roots)
        let mut queue: VecDeque<String> = indegree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(n, _)| n.clone())
            .collect();

        let mut order = Vec::new();

        while let Some(node) = queue.pop_front() {
            order.push(node.clone());

            if let Some(children) = self.edges.get(&node) {
                for child in children {
                    if let Some(entry) = indegree.get_mut(child) {
                        *entry -= 1;
                        if *entry == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }
        }

        if order.len() != self.nodes.len() {
            return Err(anyhow!("Cycle detected during topological sort"));
        }

        Ok(order)
    }

    /// Compute numeric weights for each node using the user's decay rule:
    ///
    /// weight[i] = 1.0 * (0.75^i)
    ///
    /// where i is the position in the resolved priority order.
    pub fn compute_weights(&self) -> Result<HashMap<String, f64>> {
        let order = self.resolve_order()?; // highest → lowest
        let mut weights = HashMap::new();

        for (i, node) in order.iter().enumerate() {
            let w = 1.0 * (0.75_f64.powi(i as i32));
            weights.insert(node.clone(), w);
        }

        Ok(weights)
    }

    /// Convenience: return whether the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Convenience: clear the graph.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_chain() {
        let mut pg = PriorityGraph::new();
        pg.add_edge("A", "B");
        pg.add_edge("B", "C");

        let order = pg.resolve_order().unwrap();
        assert_eq!(order, vec!["A", "B", "C"]);

        let weights = pg.compute_weights().unwrap();
        assert!(weights["A"] > weights["B"]);
        assert!(weights["B"] > weights["C"]);
    }

    #[test]
    fn detect_cycle() {
        let mut pg = PriorityGraph::new();
        pg.add_edge("A", "B");
        pg.add_edge("B", "A");

        let err = pg.resolve_order().unwrap_err();
        assert!(err.to_string().to_lowercase().contains("cycle"));
    }

    #[test]
    fn independent_nodes() {
        let mut pg = PriorityGraph::new();
        pg.add_edge("A", "B");
        pg.add_edge("C", "D");

        let order = pg.resolve_order().unwrap();
        // Order of independent chains is stable but not guaranteed
        assert!(order.contains(&"A".into()));
        assert!(order.contains(&"C".into()));
    }

    #[test]
    fn empty_graph() {
        let pg = PriorityGraph::new();
        assert!(pg.is_empty());
        let order = pg.resolve_order().unwrap_or_else(|_| vec![]);
        assert!(order.is_empty());
    }
}
