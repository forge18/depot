//! Dependency graph data structure for conflict detection

use crate::core::version::{Version, VersionConstraint};
use crate::core::{DepotError, DepotResult};
use std::collections::{HashMap, HashSet};

/// Node in the dependency graph
#[derive(Debug, Clone)]
pub struct DependencyNode {
    pub name: String,
    pub constraint: VersionConstraint,
    pub dependencies: Vec<String>,
    pub resolved_version: Option<Version>,
}

/// Dependency graph for tracking package relationships
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    nodes: HashMap<String, DependencyNode>,
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, name: String, constraint: VersionConstraint) {
        self.nodes.insert(
            name.clone(),
            DependencyNode {
                name,
                constraint,
                dependencies: Vec::new(),
                resolved_version: None,
            },
        );
    }

    /// Add a dependency edge between two nodes
    pub fn add_dependency(&mut self, from: &str, to: String) -> DepotResult<()> {
        if let Some(node) = self.nodes.get_mut(from) {
            if !node.dependencies.contains(&to) {
                node.dependencies.push(to);
            }
            Ok(())
        } else {
            Err(DepotError::Package(format!(
                "Package '{}' not found in dependency graph",
                from
            )))
        }
    }

    /// Get a node from the graph
    pub fn get_node(&self, name: &str) -> Option<&DependencyNode> {
        self.nodes.get(name)
    }

    /// Get a mutable node from the graph
    pub fn get_node_mut(&mut self, name: &str) -> Option<&mut DependencyNode> {
        self.nodes.get_mut(name)
    }

    /// Get all node names
    pub fn node_names(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Set the resolved version for a package
    pub fn set_resolved_version(&mut self, name: &str, version: Version) -> DepotResult<()> {
        if let Some(node) = self.nodes.get_mut(name) {
            node.resolved_version = Some(version);
            Ok(())
        } else {
            Err(DepotError::Package(format!(
                "Package '{}' not found in dependency graph",
                name
            )))
        }
    }

    /// Detect circular dependencies using depth-first search
    pub fn detect_circular_dependencies(&self) -> DepotResult<()> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for name in self.nodes.keys() {
            if !visited.contains(name) {
                self.detect_cycle_dfs(name, &mut visited, &mut rec_stack)?;
            }
        }

        Ok(())
    }

    /// DFS helper for cycle detection
    fn detect_cycle_dfs(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> DepotResult<()> {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());

        if let Some(node_data) = self.nodes.get(node) {
            for dep in &node_data.dependencies {
                if !visited.contains(dep) {
                    self.detect_cycle_dfs(dep, visited, rec_stack)?;
                } else if rec_stack.contains(dep) {
                    return Err(DepotError::Package(format!(
                        "Circular dependency detected: {} -> {}",
                        node, dep
                    )));
                }
            }
        }

        rec_stack.remove(node);
        Ok(())
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
    use crate::core::version::{parse_constraint, Version};

    #[test]
    fn test_new_graph() {
        let graph = DependencyGraph::new();
        assert!(graph.nodes.is_empty());
    }

    #[test]
    fn test_add_node() {
        let mut graph = DependencyGraph::new();
        let constraint = parse_constraint("^1.0.0").unwrap();
        graph.add_node("test".to_string(), constraint);
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.get_node("test").is_some());
    }

    #[test]
    fn test_add_dependency() {
        let mut graph = DependencyGraph::new();
        let constraint = parse_constraint("^1.0.0").unwrap();
        graph.add_node("pkg1".to_string(), constraint.clone());
        graph.add_node("pkg2".to_string(), constraint);

        graph.add_dependency("pkg1", "pkg2".to_string()).unwrap();

        let node = graph.get_node("pkg1").unwrap();
        assert_eq!(node.dependencies.len(), 1);
        assert_eq!(node.dependencies[0], "pkg2");
    }

    #[test]
    fn test_add_dependency_nonexistent() {
        let mut graph = DependencyGraph::new();
        let result = graph.add_dependency("nonexistent", "pkg2".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_set_resolved_version() {
        let mut graph = DependencyGraph::new();
        let constraint = parse_constraint("^1.0.0").unwrap();
        graph.add_node("test".to_string(), constraint);

        graph
            .set_resolved_version("test", Version::new(1, 2, 3))
            .unwrap();

        let node = graph.get_node("test").unwrap();
        assert_eq!(node.resolved_version, Some(Version::new(1, 2, 3)));
    }

    #[test]
    fn test_detect_circular_dependencies_simple() {
        let mut graph = DependencyGraph::new();
        let constraint = parse_constraint("^1.0.0").unwrap();

        graph.add_node("A".to_string(), constraint.clone());
        graph.add_node("B".to_string(), constraint);
        graph.add_dependency("A", "B".to_string()).unwrap();
        graph.add_dependency("B", "A".to_string()).unwrap();

        let result = graph.detect_circular_dependencies();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }

    #[test]
    fn test_detect_circular_dependencies_complex() {
        let mut graph = DependencyGraph::new();
        let constraint = parse_constraint("^1.0.0").unwrap();

        graph.add_node("A".to_string(), constraint.clone());
        graph.add_node("B".to_string(), constraint.clone());
        graph.add_node("C".to_string(), constraint);
        graph.add_dependency("A", "B".to_string()).unwrap();
        graph.add_dependency("B", "C".to_string()).unwrap();
        graph.add_dependency("C", "A".to_string()).unwrap();

        let result = graph.detect_circular_dependencies();
        assert!(result.is_err());
    }

    #[test]
    fn test_no_circular_dependencies() {
        let mut graph = DependencyGraph::new();
        let constraint = parse_constraint("^1.0.0").unwrap();

        graph.add_node("A".to_string(), constraint.clone());
        graph.add_node("B".to_string(), constraint.clone());
        graph.add_node("C".to_string(), constraint);
        graph.add_dependency("A", "B".to_string()).unwrap();
        graph.add_dependency("B", "C".to_string()).unwrap();

        let result = graph.detect_circular_dependencies();
        assert!(result.is_ok());
    }

    #[test]
    fn test_node_names() {
        let mut graph = DependencyGraph::new();
        let constraint = parse_constraint("^1.0.0").unwrap();

        graph.add_node("pkg1".to_string(), constraint.clone());
        graph.add_node("pkg2".to_string(), constraint);

        let names = graph.node_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"pkg1".to_string()));
        assert!(names.contains(&"pkg2".to_string()));
    }
}
