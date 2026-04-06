use std::collections::{HashMap, HashSet, VecDeque};

use super::error::FormulaError;

/// Tracks cell dependencies for topological evaluation ordering and
/// circular reference detection.
pub struct DependencyGraph {
    /// cell -> set of cells it depends on (forward edges).
    edges: HashMap<String, HashSet<String>>,
    /// cell -> set of cells that depend on it (reverse edges).
    reverse_edges: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    /// Record that `cell` depends on `depends_on`.
    ///
    /// E.g., if A1 = B1 + C1, call `add_dependency("A1", "B1")` and
    /// `add_dependency("A1", "C1")`.
    pub fn add_dependency(&mut self, cell: &str, depends_on: &str) {
        self.edges
            .entry(cell.to_uppercase())
            .or_default()
            .insert(depends_on.to_uppercase());

        self.reverse_edges
            .entry(depends_on.to_uppercase())
            .or_default()
            .insert(cell.to_uppercase());
    }

    /// Remove all dependencies for a cell (e.g., when its formula changes).
    pub fn remove_cell(&mut self, cell: &str) {
        let cell = cell.to_uppercase();

        // Remove forward edges and corresponding reverse edges.
        if let Some(deps) = self.edges.remove(&cell) {
            for dep in &deps {
                if let Some(rev) = self.reverse_edges.get_mut(dep) {
                    rev.remove(&cell);
                    if rev.is_empty() {
                        self.reverse_edges.remove(dep);
                    }
                }
            }
        }

        // Remove reverse edges where this cell was a dependency.
        if let Some(dependents) = self.reverse_edges.remove(&cell) {
            for dependent in &dependents {
                if let Some(fwd) = self.edges.get_mut(dependent) {
                    fwd.remove(&cell);
                    if fwd.is_empty() {
                        self.edges.remove(dependent);
                    }
                }
            }
        }
    }

    /// Return all cells that directly or indirectly depend on the given cell.
    pub fn dependents(&self, cell: &str) -> HashSet<String> {
        let cell = cell.to_uppercase();
        let mut result = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(cell.clone());

        while let Some(current) = queue.pop_front() {
            if let Some(deps) = self.reverse_edges.get(&current) {
                for dep in deps {
                    if result.insert(dep.clone()) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        result
    }

    /// Check if adding or evaluating `cell` would create a circular reference.
    ///
    /// Uses DFS from `cell` following forward edges. If we reach `cell` again,
    /// there's a cycle.
    pub fn has_circular(&self, cell: &str) -> bool {
        let cell = cell.to_uppercase();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();

        // Start from the dependencies of the given cell.
        if let Some(deps) = self.edges.get(&cell) {
            for dep in deps {
                stack.push(dep.clone());
            }
        }

        while let Some(current) = stack.pop() {
            if current == cell {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(deps) = self.edges.get(&current) {
                    for dep in deps {
                        stack.push(dep.clone());
                    }
                }
            }
        }

        false
    }

    /// Compute a topological evaluation order (cells with no dependencies first).
    ///
    /// Returns an error if a circular reference is detected.
    pub fn evaluation_order(&self) -> Result<Vec<String>, FormulaError> {
        // Kahn's algorithm.
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Collect all nodes.
        let mut all_nodes = HashSet::new();
        for (cell, deps) in &self.edges {
            all_nodes.insert(cell.clone());
            for dep in deps {
                all_nodes.insert(dep.clone());
            }
        }

        // In our model, edges[A] = {B, C} means A depends on B and C,
        // so the evaluation edge goes B -> A (B must be evaluated before A).
        // Therefore A's in-degree = number of its dependencies.
        for node in &all_nodes {
            let count = self.edges.get(node).map(|s| s.len()).unwrap_or(0);
            in_degree.insert(node.clone(), count);
        }

        let mut queue: VecDeque<String> = VecDeque::new();
        for (node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node.clone());
            }
        }

        let mut order = Vec::new();

        while let Some(current) = queue.pop_front() {
            order.push(current.clone());

            // For each cell that depends on `current`, decrease its in-degree.
            if let Some(dependents) = self.reverse_edges.get(&current) {
                for dependent in dependents {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        if order.len() != all_nodes.len() {
            Err(FormulaError::CircularReference)
        } else {
            Ok(order)
        }
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let graph = DependencyGraph::new();
        assert!(!graph.has_circular("A1"));
        let order = graph.evaluation_order().unwrap();
        assert!(order.is_empty());
    }

    #[test]
    fn simple_chain() {
        // A1 depends on B1, B1 depends on C1
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "B1");
        graph.add_dependency("B1", "C1");

        assert!(!graph.has_circular("A1"));
        assert!(!graph.has_circular("B1"));

        let order = graph.evaluation_order().unwrap();
        // C1 must come before B1, B1 must come before A1
        let pos = |name: &str| order.iter().position(|x| x == name).unwrap();
        assert!(pos("C1") < pos("B1"));
        assert!(pos("B1") < pos("A1"));
    }

    #[test]
    fn circular_reference() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "B1");
        graph.add_dependency("B1", "A1");

        assert!(graph.has_circular("A1"));
        assert!(graph.has_circular("B1"));
        assert!(graph.evaluation_order().is_err());
    }

    #[test]
    fn indirect_circular_reference() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "B1");
        graph.add_dependency("B1", "C1");
        graph.add_dependency("C1", "A1");

        assert!(graph.has_circular("A1"));
        assert!(graph.has_circular("B1"));
        assert!(graph.has_circular("C1"));
    }

    #[test]
    fn no_circular_with_shared_dep() {
        // A1 depends on C1, B1 depends on C1 — no cycle
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "C1");
        graph.add_dependency("B1", "C1");

        assert!(!graph.has_circular("A1"));
        assert!(!graph.has_circular("B1"));
    }

    #[test]
    fn dependents_direct() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "B1");
        graph.add_dependency("C1", "B1");

        let deps = graph.dependents("B1");
        assert!(deps.contains("A1"));
        assert!(deps.contains("C1"));
        assert!(!deps.contains("B1"));
    }

    #[test]
    fn dependents_transitive() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "B1");
        graph.add_dependency("B1", "C1");

        let deps = graph.dependents("C1");
        assert!(deps.contains("B1"));
        assert!(deps.contains("A1"));
    }

    #[test]
    fn remove_cell() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "B1");
        graph.add_dependency("A1", "C1");
        graph.add_dependency("D1", "A1");

        graph.remove_cell("A1");

        // A1's dependencies should be gone
        assert!(graph.dependents("B1").is_empty());
        assert!(graph.dependents("C1").is_empty());
        // D1's dependency on A1 should also be cleaned up
        assert!(!graph.has_circular("D1"));
    }

    #[test]
    fn evaluation_order_diamond() {
        //   A1
        //  / \
        // B1  C1
        //  \ /
        //   D1
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "B1");
        graph.add_dependency("A1", "C1");
        graph.add_dependency("B1", "D1");
        graph.add_dependency("C1", "D1");

        let order = graph.evaluation_order().unwrap();
        let pos = |name: &str| order.iter().position(|x| x == name).unwrap();

        assert!(pos("D1") < pos("B1"));
        assert!(pos("D1") < pos("C1"));
        assert!(pos("B1") < pos("A1"));
        assert!(pos("C1") < pos("A1"));
    }

    #[test]
    fn case_insensitive() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("a1", "b1");
        assert!(graph.dependents("B1").contains("A1"));
    }

    #[test]
    fn self_reference() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("A1", "A1");
        assert!(graph.has_circular("A1"));
    }
}
