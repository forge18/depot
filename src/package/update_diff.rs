use crate::core::version::Version;
use crate::package::lockfile::Lockfile;
use std::collections::HashMap;
use std::path::Path;

/// Represents a change to a package during update
#[derive(Debug, Clone)]
pub enum PackageChange {
    /// Package will be updated to a new version
    Updated {
        name: String,
        current_version: Version,
        new_version: Version,
    },
    /// Package will be newly installed
    Added { name: String, version: Version },
    /// Package will be removed
    Removed { name: String, version: Version },
    /// Package is already up to date
    UpToDate { name: String, version: Version },
}

/// Represents file changes for a package update
#[derive(Debug, Clone)]
pub struct PackageFileChanges {
    pub package_name: String,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

/// Summary of all changes that will be made during update
#[derive(Debug, Clone)]
pub struct UpdateDiff {
    pub package_changes: Vec<PackageChange>,
    pub file_changes: Vec<PackageFileChanges>,
}

impl UpdateDiff {
    pub fn new() -> Self {
        Self {
            package_changes: Vec::new(),
            file_changes: Vec::new(),
        }
    }

    /// Calculate the diff between current lockfile and resolved versions
    pub fn calculate(
        current_lockfile: &Option<Lockfile>,
        resolved_versions: &HashMap<String, Version>,
        resolved_dev_versions: &HashMap<String, Version>,
    ) -> Self {
        let mut diff = Self::new();

        // Check regular dependencies
        for (name, new_version) in resolved_versions {
            if let Some(lockfile) = current_lockfile {
                if let Some(locked_pkg) = lockfile.get_package(name) {
                    if let Ok(current_version) = Version::parse(&locked_pkg.version) {
                        if current_version == *new_version {
                            diff.package_changes.push(PackageChange::UpToDate {
                                name: name.clone(),
                                version: new_version.clone(),
                            });
                        } else {
                            diff.package_changes.push(PackageChange::Updated {
                                name: name.clone(),
                                current_version,
                                new_version: new_version.clone(),
                            });
                        }
                    } else {
                        // Can't parse current version, treat as update
                        diff.package_changes.push(PackageChange::Updated {
                            name: name.clone(),
                            current_version: Version::new(0, 0, 0), // Unknown
                            new_version: new_version.clone(),
                        });
                    }
                } else {
                    // Not in lockfile, it's new
                    diff.package_changes.push(PackageChange::Added {
                        name: name.clone(),
                        version: new_version.clone(),
                    });
                }
            } else {
                // No lockfile, everything is new
                diff.package_changes.push(PackageChange::Added {
                    name: name.clone(),
                    version: new_version.clone(),
                });
            }
        }

        // Check dev dependencies
        for (name, new_version) in resolved_dev_versions {
            if let Some(lockfile) = current_lockfile {
                if let Some(locked_pkg) = lockfile.get_package(name) {
                    if let Ok(current_version) = Version::parse(&locked_pkg.version) {
                        if current_version != *new_version {
                            diff.package_changes.push(PackageChange::Updated {
                                name: format!("{} (dev)", name),
                                current_version,
                                new_version: new_version.clone(),
                            });
                        }
                    }
                } else {
                    diff.package_changes.push(PackageChange::Added {
                        name: format!("{} (dev)", name),
                        version: new_version.clone(),
                    });
                }
            } else {
                diff.package_changes.push(PackageChange::Added {
                    name: format!("{} (dev)", name),
                    version: new_version.clone(),
                });
            }
        }

        // Check for removed packages (in lockfile but not in resolved)
        if let Some(lockfile) = current_lockfile {
            for (name, locked_pkg) in &lockfile.packages {
                // Skip if it's in resolved versions (already handled above)
                if !resolved_versions.contains_key(name)
                    && !resolved_dev_versions.contains_key(name)
                {
                    if let Ok(version) = Version::parse(&locked_pkg.version) {
                        diff.package_changes.push(PackageChange::Removed {
                            name: name.clone(),
                            version,
                        });
                    }
                }
            }
        }

        diff
    }

    /// Calculate file changes for packages that will be updated
    pub fn calculate_file_changes(&mut self, project_root: &Path) {
        use crate::core::path::lua_modules_dir;
        use std::fs;

        let lua_modules = lua_modules_dir(project_root);

        for change in &self.package_changes {
            match change {
                PackageChange::Updated { name, .. } | PackageChange::Added { name, .. } => {
                    let package_dir = lua_modules.join(name);
                    let mut file_changes = PackageFileChanges {
                        package_name: name.clone(),
                        added: Vec::new(),
                        modified: Vec::new(),
                        deleted: Vec::new(),
                    };

                    // If package is being updated and already exists, check for file changes
                    if let PackageChange::Updated { .. } = change {
                        if package_dir.exists() {
                            // Get list of current files
                            let current_files: Vec<String> =
                                if let Ok(entries) = fs::read_dir(&package_dir) {
                                    entries
                                        .filter_map(|e| e.ok())
                                        .filter_map(|e| {
                                            e.path()
                                                .strip_prefix(&package_dir)
                                                .ok()
                                                .and_then(|p| p.to_str().map(|s| s.to_string()))
                                        })
                                        .collect()
                                } else {
                                    Vec::new()
                                };

                            // For now, we'll mark all files as potentially modified
                            // In a full implementation, we'd compare checksums or file contents
                            file_changes.modified = current_files;
                        }
                    } else {
                        // New package - files will be added
                        // We can't know the exact files until download, but we can note it
                        file_changes.added.push("(package files)".to_string());
                    }

                    if !file_changes.added.is_empty()
                        || !file_changes.modified.is_empty()
                        || !file_changes.deleted.is_empty()
                    {
                        self.file_changes.push(file_changes);
                    }
                }
                PackageChange::Removed { name, .. } => {
                    let package_dir = lua_modules.join(name);
                    let mut file_changes = PackageFileChanges {
                        package_name: name.clone(),
                        added: Vec::new(),
                        modified: Vec::new(),
                        deleted: Vec::new(),
                    };

                    if package_dir.exists() {
                        // Get list of files that will be deleted
                        if let Ok(entries) = fs::read_dir(&package_dir) {
                            file_changes.deleted = entries
                                .filter_map(|e| e.ok())
                                .filter_map(|e| {
                                    e.path()
                                        .strip_prefix(&package_dir)
                                        .ok()
                                        .and_then(|p| p.to_str().map(|s| s.to_string()))
                                })
                                .collect();
                        }
                    }

                    if !file_changes.deleted.is_empty() {
                        self.file_changes.push(file_changes);
                    }
                }
                _ => {}
            }
        }
    }

    /// Display the diff in a human-readable format
    pub fn display(&self) {
        println!("\n📦 Update Summary:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mut updated_count = 0;
        let mut added_count = 0;
        let mut removed_count = 0;
        let mut up_to_date_count = 0;

        for change in &self.package_changes {
            match change {
                PackageChange::Updated {
                    name,
                    current_version,
                    new_version,
                } => {
                    println!("  ⬆️  {}: {} → {}", name, current_version, new_version);
                    updated_count += 1;
                }
                PackageChange::Added { name, version } => {
                    println!("  ➕ {}: {} (new)", name, version);
                    added_count += 1;
                }
                PackageChange::Removed { name, version } => {
                    println!("  ➖ {}: {} (removed)", name, version);
                    removed_count += 1;
                }
                PackageChange::UpToDate { name, version } => {
                    println!("  ✓ {}: {} (up to date)", name, version);
                    up_to_date_count += 1;
                }
            }
        }

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!(
            "  Updated: {} | Added: {} | Removed: {} | Up to date: {}",
            updated_count, added_count, removed_count, up_to_date_count
        );

        // Display file changes if any
        if !self.file_changes.is_empty() {
            println!("\n📁 File Changes:");
            for file_change in &self.file_changes {
                println!("\n  Package: {}", file_change.package_name);
                if !file_change.added.is_empty() {
                    println!("    Added:");
                    for file in &file_change.added {
                        println!("      + {}", file);
                    }
                }
                if !file_change.modified.is_empty() {
                    println!("    Modified:");
                    for file in &file_change.modified {
                        println!("      ~ {}", file);
                    }
                }
                if !file_change.deleted.is_empty() {
                    println!("    Deleted:");
                    for file in &file_change.deleted {
                        println!("      - {}", file);
                    }
                }
            }
        }
    }

    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        self.package_changes.iter().any(|c| {
            matches!(
                c,
                PackageChange::Updated { .. }
                    | PackageChange::Added { .. }
                    | PackageChange::Removed { .. }
            )
        })
    }
}

impl Default for UpdateDiff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::lockfile::{LockedPackage, Lockfile};
    use std::collections::HashMap;

    fn create_test_lockfile() -> Lockfile {
        let mut lockfile = Lockfile::new();
        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "abc123".to_string(),
            size: None,
            dependencies: HashMap::new(),
            build: None,
        };
        lockfile.add_package("test-package".to_string(), package);
        lockfile
    }

    #[test]
    fn test_update_diff_new() {
        let diff = UpdateDiff::new();
        assert!(diff.package_changes.is_empty());
        assert!(diff.file_changes.is_empty());
    }

    #[test]
    fn test_update_diff_calculate_no_lockfile() {
        let resolved = HashMap::from([("new-package".to_string(), Version::new(1, 0, 0))]);

        let diff = UpdateDiff::calculate(&None, &resolved, &HashMap::new());
        assert_eq!(diff.package_changes.len(), 1);
        match &diff.package_changes[0] {
            PackageChange::Added { name, version } => {
                assert_eq!(name, "new-package");
                assert_eq!(version, &Version::new(1, 0, 0));
            }
            _ => panic!("Expected Added change"),
        }
    }

    #[test]
    fn test_update_diff_calculate_up_to_date() {
        let lockfile = create_test_lockfile();
        let resolved = HashMap::from([("test-package".to_string(), Version::new(1, 0, 0))]);

        let diff = UpdateDiff::calculate(&Some(lockfile), &resolved, &HashMap::new());
        assert_eq!(diff.package_changes.len(), 1);
        match &diff.package_changes[0] {
            PackageChange::UpToDate { name, version } => {
                assert_eq!(name, "test-package");
                assert_eq!(version, &Version::new(1, 0, 0));
            }
            _ => panic!("Expected UpToDate change"),
        }
    }

    #[test]
    fn test_update_diff_calculate_updated() {
        let lockfile = create_test_lockfile();
        let mut resolved = HashMap::new();
        resolved.insert("test-package".to_string(), Version::new(2, 0, 0));

        let diff = UpdateDiff::calculate(&Some(lockfile), &resolved, &HashMap::new());
        assert_eq!(diff.package_changes.len(), 1);
        match &diff.package_changes[0] {
            PackageChange::Updated {
                name,
                current_version,
                new_version,
            } => {
                assert_eq!(name, "test-package");
                assert_eq!(current_version, &Version::new(1, 0, 0));
                assert_eq!(new_version, &Version::new(2, 0, 0));
            }
            _ => panic!("Expected Updated change"),
        }
    }

    #[test]
    fn test_update_diff_calculate_removed() {
        let lockfile = create_test_lockfile();
        let resolved = HashMap::new();

        let diff = UpdateDiff::calculate(&Some(lockfile), &resolved, &HashMap::new());
        assert_eq!(diff.package_changes.len(), 1);
        match &diff.package_changes[0] {
            PackageChange::Removed { name, version } => {
                assert_eq!(name, "test-package");
                assert_eq!(version, &Version::new(1, 0, 0));
            }
            _ => panic!("Expected Removed change"),
        }
    }

    #[test]
    fn test_update_diff_has_changes() {
        let mut diff = UpdateDiff::new();
        assert!(!diff.has_changes());

        diff.package_changes.push(PackageChange::UpToDate {
            name: "test".to_string(),
            version: Version::new(1, 0, 0),
        });
        assert!(!diff.has_changes());

        diff.package_changes.push(PackageChange::Updated {
            name: "test".to_string(),
            current_version: Version::new(1, 0, 0),
            new_version: Version::new(2, 0, 0),
        });
        assert!(diff.has_changes());
    }

    #[test]
    fn test_update_diff_with_dev_dependencies() {
        let lockfile = create_test_lockfile();
        let resolved = HashMap::new();
        let resolved_dev = HashMap::from([("dev-package".to_string(), Version::new(1, 0, 0))]);

        let diff = UpdateDiff::calculate(&Some(lockfile), &resolved, &resolved_dev);
        assert_eq!(diff.package_changes.len(), 2); // removed test-package + added dev-package
    }

    #[test]
    fn test_update_diff_calculate_dev_dependency_update() {
        let mut lockfile = Lockfile::new();
        let dev_package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "abc123".to_string(),
            size: None,
            dependencies: HashMap::new(),
            build: None,
        };
        lockfile.add_package("dev-package".to_string(), dev_package);

        let resolved = HashMap::new();
        let resolved_dev = HashMap::from([("dev-package".to_string(), Version::new(2, 0, 0))]);

        let diff = UpdateDiff::calculate(&Some(lockfile), &resolved, &resolved_dev);
        assert_eq!(diff.package_changes.len(), 1);
        match &diff.package_changes[0] {
            PackageChange::Updated {
                name,
                current_version,
                new_version,
            } => {
                assert!(name.contains("dev-package"));
                assert_eq!(current_version, &Version::new(1, 0, 0));
                assert_eq!(new_version, &Version::new(2, 0, 0));
            }
            _ => panic!("Expected Updated change for dev dependency"),
        }
    }

    #[test]
    fn test_update_diff_calculate_file_changes_for_added() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::Added {
            name: "new-package".to_string(),
            version: Version::new(1, 0, 0),
        });

        diff.calculate_file_changes(temp.path());

        // Should have file changes for added package
        assert!(!diff.file_changes.is_empty());
        assert_eq!(diff.file_changes[0].package_name, "new-package");
        assert!(!diff.file_changes[0].added.is_empty());
    }

    #[test]
    fn test_update_diff_calculate_file_changes_for_updated() {
        let temp = tempfile::TempDir::new().unwrap();

        // Create existing package directory
        use crate::core::path::lua_modules_dir;
        let lua_modules = lua_modules_dir(temp.path());
        let package_dir = lua_modules.join("updated-package");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(package_dir.join("file1.lua"), "content1").unwrap();
        std::fs::write(package_dir.join("file2.lua"), "content2").unwrap();

        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::Updated {
            name: "updated-package".to_string(),
            current_version: Version::new(1, 0, 0),
            new_version: Version::new(2, 0, 0),
        });

        diff.calculate_file_changes(temp.path());

        // Should detect modified files
        let file_change = diff
            .file_changes
            .iter()
            .find(|fc| fc.package_name == "updated-package");
        assert!(file_change.is_some());
        let file_change = file_change.unwrap();
        assert!(!file_change.modified.is_empty());
    }

    #[test]
    fn test_update_diff_calculate_file_changes_for_removed() {
        let temp = tempfile::TempDir::new().unwrap();

        // Create existing package directory
        use crate::core::path::lua_modules_dir;
        let lua_modules = lua_modules_dir(temp.path());
        let package_dir = lua_modules.join("removed-package");
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(package_dir.join("file1.lua"), "content1").unwrap();

        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::Removed {
            name: "removed-package".to_string(),
            version: Version::new(1, 0, 0),
        });

        diff.calculate_file_changes(temp.path());

        // Should detect deleted files
        let file_change = diff
            .file_changes
            .iter()
            .find(|fc| fc.package_name == "removed-package");
        assert!(file_change.is_some());
        let file_change = file_change.unwrap();
        assert!(!file_change.deleted.is_empty());
    }

    #[test]
    fn test_update_diff_display() {
        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::Updated {
            name: "pkg1".to_string(),
            current_version: Version::new(1, 0, 0),
            new_version: Version::new(2, 0, 0),
        });
        diff.package_changes.push(PackageChange::Added {
            name: "pkg2".to_string(),
            version: Version::new(1, 0, 0),
        });
        diff.package_changes.push(PackageChange::Removed {
            name: "pkg3".to_string(),
            version: Version::new(1, 0, 0),
        });
        diff.package_changes.push(PackageChange::UpToDate {
            name: "pkg4".to_string(),
            version: Version::new(1, 0, 0),
        });

        // Should not panic
        diff.display();
    }

    #[test]
    fn test_update_diff_display_with_file_changes() {
        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::Updated {
            name: "pkg1".to_string(),
            current_version: Version::new(1, 0, 0),
            new_version: Version::new(2, 0, 0),
        });
        diff.file_changes.push(PackageFileChanges {
            package_name: "pkg1".to_string(),
            added: vec!["new.lua".to_string()],
            modified: vec!["existing.lua".to_string()],
            deleted: vec!["old.lua".to_string()],
        });

        // Should not panic
        diff.display();
    }

    #[test]
    fn test_update_diff_default() {
        let diff = UpdateDiff::default();
        assert!(diff.package_changes.is_empty());
        assert!(diff.file_changes.is_empty());
    }

    #[test]
    fn test_update_diff_calculate_invalid_version() {
        let mut lockfile = Lockfile::new();
        let package = LockedPackage {
            version: "invalid".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "abc123".to_string(),
            size: None,
            dependencies: HashMap::new(),
            build: None,
        };
        lockfile.add_package("test-package".to_string(), package);
        let resolved = HashMap::from([("test-package".to_string(), Version::new(2, 0, 0))]);
        let diff = UpdateDiff::calculate(&Some(lockfile), &resolved, &HashMap::new());
        assert_eq!(diff.package_changes.len(), 1);
        match &diff.package_changes[0] {
            PackageChange::Updated {
                name,
                current_version,
                new_version,
            } => {
                assert_eq!(name, "test-package");
                assert_eq!(current_version, &Version::new(0, 0, 0));
                assert_eq!(new_version, &Version::new(2, 0, 0));
            }
            _ => panic!("Expected Updated change"),
        }
    }

    #[test]
    fn test_update_diff_calculate_file_changes_up_to_date() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::UpToDate {
            name: "pkg".to_string(),
            version: Version::new(1, 0, 0),
        });
        diff.calculate_file_changes(temp.path());
        assert!(diff.file_changes.is_empty());
    }

    #[test]
    fn test_update_diff_calculate_file_changes_removed_no_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::Removed {
            name: "nonexistent".to_string(),
            version: Version::new(1, 0, 0),
        });
        diff.calculate_file_changes(temp.path());
        assert!(diff.file_changes.is_empty());
    }

    #[test]
    fn test_update_diff_display_empty() {
        let diff = UpdateDiff::new();
        diff.display(); // Should not panic
    }

    #[test]
    fn test_update_diff_display_with_changes() {
        let mut diff = UpdateDiff::new();
        diff.package_changes.push(PackageChange::Updated {
            name: "test-pkg".to_string(),
            current_version: Version::new(1, 0, 0),
            new_version: Version::new(2, 0, 0),
        });
        diff.package_changes.push(PackageChange::Added {
            name: "new-pkg".to_string(),
            version: Version::new(1, 0, 0),
        });
        diff.display(); // Should not panic
    }

    #[test]
    fn test_update_diff_calculate_dev_dependency_up_to_date() {
        let mut lockfile = Lockfile::new();
        let package = LockedPackage {
            version: "1.0.0".to_string(),
            source: "luarocks".to_string(),
            rockspec_url: None,
            source_url: None,
            checksum: "abc123".to_string(),
            size: None,
            dependencies: HashMap::new(),
            build: None,
        };
        lockfile.add_package("dev-pkg".to_string(), package);
        let resolved = HashMap::new();
        let resolved_dev = HashMap::from([("dev-pkg".to_string(), Version::new(1, 0, 0))]);
        let diff = UpdateDiff::calculate(&Some(lockfile), &resolved, &resolved_dev);
        // Should not have changes for up-to-date dev dependency
        assert!(!diff.has_changes());
    }
}
