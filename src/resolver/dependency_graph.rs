use crate::core::version::{Version, VersionConstraint};
use crate::core::{LpmError, LpmResult};
use std::collections::{HashMap, HashSet};

/// A node in the dependency graph
#[derive(Debug, Clone)]
pub struct DependencyNode {
    pub name: String,
    pub constraint: VersionConstraint,
    pub resolved_version: Option<Version>,
    pub dependencies: Vec<String>, // Names of dependencies
}

/// Dependency graph for version resolution
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    nodes: HashMap<String, DependencyNode>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Add a dependency node to the graph
    pub fn add_node(&mut self, name: String, constraint: VersionConstraint) {
        self.nodes.insert(
            name.clone(),
            DependencyNode {
                name,
                constraint,
                resolved_version: None,
                dependencies: Vec::new(),
            },
        );
    }

    /// Add a dependency edge (package A depends on package B)
    pub fn add_dependency(&mut self, package: &str, dependency: String) -> LpmResult<()> {
        if let Some(node) = self.nodes.get_mut(package) {
            node.dependencies.push(dependency);
            Ok(())
        } else {
            Err(LpmError::Package(format!(
                "Package '{}' not found in graph",
                package
            )))
        }
    }

    /// Get a node by name
    pub fn get_node(&self, name: &str) -> Option<&DependencyNode> {
        self.nodes.get(name)
    }

    /// Get all node names
    pub fn node_names(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Set the resolved version for a node
    pub fn set_resolved_version(&mut self, name: &str, version: Version) -> LpmResult<()> {
        if let Some(node) = self.nodes.get_mut(name) {
            node.resolved_version = Some(version);
            Ok(())
        } else {
            Err(LpmError::Package(format!(
                "Package '{}' not found in graph",
                name
            )))
        }
    }

    /// Detect circular dependencies using DFS
    pub fn detect_circular_dependencies(&self) -> LpmResult<Vec<Vec<String>>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for node_name in self.nodes.keys() {
            if !visited.contains(node_name) {
                self.dfs_detect_cycles(
                    node_name,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    &mut cycles,
                )?;
            }
        }

        if cycles.is_empty() {
            Ok(cycles)
        } else {
            Err(LpmError::Package(format!(
                "Circular dependencies detected: {:?}",
                cycles
            )))
        }
    }

    fn dfs_detect_cycles(
        &self,
        node_name: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) -> LpmResult<()> {
        visited.insert(node_name.to_string());
        rec_stack.insert(node_name.to_string());
        path.push(node_name.to_string());

        if let Some(node) = self.nodes.get(node_name) {
            for dep in &node.dependencies {
                if !visited.contains(dep) {
                    self.dfs_detect_cycles(dep, visited, rec_stack, path, cycles)?;
                } else if rec_stack.contains(dep) {
                    // Found a cycle - extract the cycle path
                    let cycle_start = path.iter().position(|n| n == dep).unwrap();
                    let cycle: Vec<String> = path[cycle_start..].to_vec();
                    cycles.push(cycle);
                }
            }
        }

        rec_stack.remove(node_name);
        path.pop();
        Ok(())
    }

    /// Get all dependencies of a package (transitive)
    pub fn get_all_dependencies(&self, package: &str) -> HashSet<String> {
        let mut deps = HashSet::new();
        let mut to_process = vec![package.to_string()];

        while let Some(current) = to_process.pop() {
            if let Some(node) = self.nodes.get(&current) {
                for dep in &node.dependencies {
                    if !deps.contains(dep) {
                        deps.insert(dep.clone());
                        to_process.push(dep.clone());
                    }
                }
            }
        }

        deps
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
    use crate::core::version::parse_constraint;

    #[test]
    fn test_add_node() {
        let mut graph = DependencyGraph::new();
        graph.add_node("test".to_string(), parse_constraint("^1.0.0").unwrap());
        assert!(graph.get_node("test").is_some());
    }

    #[test]
    fn test_add_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("b".to_string(), parse_constraint("^2.0.0").unwrap());
        graph.add_dependency("a", "b".to_string()).unwrap();

        let node = graph.get_node("a").unwrap();
        assert_eq!(node.dependencies.len(), 1);
        assert_eq!(node.dependencies[0], "b");
    }

    #[test]
    fn test_detect_circular_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("b".to_string(), parse_constraint("^2.0.0").unwrap());
        graph.add_dependency("a", "b".to_string()).unwrap();
        graph.add_dependency("b", "a".to_string()).unwrap();

        let cycles = graph.detect_circular_dependencies();
        assert!(cycles.is_err());
    }

    #[test]
    fn test_no_circular_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("b".to_string(), parse_constraint("^2.0.0").unwrap());
        graph.add_node("c".to_string(), parse_constraint("^3.0.0").unwrap());
        graph.add_dependency("a", "b".to_string()).unwrap();
        graph.add_dependency("b", "c".to_string()).unwrap();

        let cycles = graph.detect_circular_dependencies();
        assert!(cycles.is_ok());
        assert!(cycles.unwrap().is_empty());
    }

    #[test]
    fn test_add_dependency_nonexistent_package() {
        let mut graph = DependencyGraph::new();
        graph.add_node("package-a".to_string(), parse_constraint("^1.0.0").unwrap());

        let result = graph.add_dependency("nonexistent", "package-b".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_get_node() {
        let mut graph = DependencyGraph::new();
        graph.add_node(
            "test-package".to_string(),
            parse_constraint("^1.0.0").unwrap(),
        );

        let node = graph.get_node("test-package");
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "test-package");

        let none = graph.get_node("nonexistent");
        assert!(none.is_none());
    }

    #[test]
    fn test_set_resolved_version() {
        let mut graph = DependencyGraph::new();
        graph.add_node(
            "test-package".to_string(),
            parse_constraint("^1.0.0").unwrap(),
        );

        graph
            .set_resolved_version("test-package", Version::new(2, 0, 0))
            .unwrap();

        let node = graph.get_node("test-package").unwrap();
        assert_eq!(node.resolved_version, Some(Version::new(2, 0, 0)));
    }

    #[test]
    fn test_set_resolved_version_nonexistent() {
        let mut graph = DependencyGraph::new();
        let result = graph.set_resolved_version("nonexistent", Version::new(1, 0, 0));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_all_dependencies() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("b".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_node("c".to_string(), parse_constraint("^1.0.0").unwrap());
        graph.add_dependency("a", "b".to_string()).unwrap();
        graph.add_dependency("b", "c".to_string()).unwrap();

        let deps = graph.get_all_dependencies("a");
        assert!(deps.contains("b"));
        assert!(deps.contains("c"));
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_get_all_dependencies_nonexistent() {
        let graph = DependencyGraph::new();
        let deps = graph.get_all_dependencies("nonexistent");
        assert!(deps.is_empty());
    }

    #[test]
    fn test_get_all_dependencies_no_deps() {
        let mut graph = DependencyGraph::new();
        graph.add_node("a".to_string(), parse_constraint("^1.0.0").unwrap());

        let deps = graph.get_all_dependencies("a");
        assert!(deps.is_empty());
    }

    #[test]
    fn test_dependency_graph_default() {
        let graph = DependencyGraph::default();
        assert!(graph.nodes.is_empty());
    }
}
