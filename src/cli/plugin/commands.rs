use crate::cli::plugin::config::PluginConfig;
use crate::cli::plugin::installer::PluginInstaller;
use crate::cli::plugin::registry::PluginRegistry;
use crate::cli::plugin::{list_plugins, PluginInfo};
use clap::{Parser, Subcommand};
use lpm_core::{LpmError, LpmResult};
use tokio::runtime::Runtime;

/// Plugin management commands
#[derive(Parser)]
pub struct PluginCommands {
    #[command(subcommand)]
    pub command: PluginSubcommand,
}

#[derive(Subcommand)]
pub enum PluginSubcommand {
    /// List installed plugins
    List,
    /// Show plugin information
    Info {
        /// Plugin name
        name: String,
    },
    /// Update plugins
    Update {
        /// Plugin names to update (updates all if not specified)
        names: Option<Vec<String>>,
    },
    /// Check for plugin updates
    Outdated,
    /// Search plugin registry
    Search {
        /// Search query
        query: Option<String>,
    },
    /// Manage plugin configuration
    Config {
        #[command(subcommand)]
        command: ConfigSubcommand,
    },
}

#[derive(Subcommand)]
pub enum ConfigSubcommand {
    /// Get a configuration value
    Get {
        /// Plugin name
        plugin: String,
        /// Configuration key
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Plugin name
        plugin: String,
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Show all configuration for a plugin
    Show {
        /// Plugin name
        plugin: String,
    },
}

/// Run plugin command
pub fn run(command: PluginSubcommand) -> LpmResult<()> {
    match command {
        PluginSubcommand::List => list_installed_plugins(),
        PluginSubcommand::Info { name } => show_plugin_info(&name),
        PluginSubcommand::Update { names } => update_plugins(names),
        PluginSubcommand::Outdated => check_outdated_plugins(),
        PluginSubcommand::Search { query } => search_plugins(query),
        PluginSubcommand::Config { command } => run_config_command(command),
    }
}

fn run_config_command(command: ConfigSubcommand) -> LpmResult<()> {
    match command {
        ConfigSubcommand::Get { plugin, key } => {
            let config = PluginConfig::load(&plugin)?;
            if let Some(value) = config.get::<String>(&key) {
                println!("{}", value);
            } else if let Some(value) = config.get::<i64>(&key) {
                println!("{}", value);
            } else if let Some(value) = config.get::<bool>(&key) {
                println!("{}", value);
            } else {
                println!(
                    "Key '{}' not found in plugin '{}' configuration",
                    key, plugin
                );
            }
            Ok(())
        }
        ConfigSubcommand::Set { plugin, key, value } => {
            let mut config = PluginConfig::load(&plugin)?;
            // Try to parse as number, then bool, then string
            if let Ok(num) = value.parse::<i64>() {
                config.set(key, num)?;
            } else if let Ok(b) = value.parse::<bool>() {
                config.set(key, b)?;
            } else {
                config.set(key, value)?;
            }
            config.save()?;
            println!("Configuration updated for plugin '{}'", plugin);
            Ok(())
        }
        ConfigSubcommand::Show { plugin } => {
            let config = PluginConfig::load(&plugin)?;
            if config.settings.is_empty() {
                println!("No configuration found for plugin '{}'", plugin);
            } else {
                println!("Configuration for plugin '{}':", plugin);
                for (key, value) in &config.settings {
                    println!("  {} = {:?}", key, value);
                }
            }
            Ok(())
        }
    }
}

fn list_installed_plugins() -> LpmResult<()> {
    let plugins = list_plugins()?;

    if plugins.is_empty() {
        println!("No plugins installed.");
        println!("\nInstall plugins with: lpm install -g lpm-<name>");
        return Ok(());
    }

    println!("Installed plugins:\n");
    for plugin in plugins {
        let version = plugin
            .installed_version
            .as_ref()
            .unwrap_or(&plugin.metadata.version);
        println!("  {} {}", plugin.metadata.name, version);
        if let Some(desc) = &plugin.metadata.description {
            println!("    {}", desc);
        }
    }

    Ok(())
}

fn show_plugin_info(name: &str) -> LpmResult<()> {
    if let Some(plugin) = PluginInfo::from_installed(name)? {
        println!("Plugin: {}", plugin.metadata.name);
        println!(
            "Version: {}",
            plugin
                .installed_version
                .as_ref()
                .unwrap_or(&plugin.metadata.version)
        );

        if let Some(desc) = &plugin.metadata.description {
            println!("Description: {}", desc);
        }
        if let Some(author) = &plugin.metadata.author {
            println!("Author: {}", author);
        }
        if let Some(homepage) = &plugin.metadata.homepage {
            println!("Homepage: {}", homepage);
        }

        println!("Executable: {}", plugin.executable_path.display());

        if !plugin.metadata.dependencies.is_empty() {
            println!("\nDependencies:");
            for dep in &plugin.metadata.dependencies {
                if let Some(version) = &dep.version {
                    println!("  - {} ({})", dep.name, version);
                } else {
                    println!("  - {}", dep.name);
                }
            }
        }
    } else {
        return Err(LpmError::Package(format!(
            "Plugin '{}' is not installed",
            name
        )));
    }

    Ok(())
}

fn update_plugins(names: Option<Vec<String>>) -> LpmResult<()> {
    let rt = Runtime::new()
        .map_err(|e| LpmError::Package(format!("Failed to create runtime: {}", e)))?;

    if let Some(plugin_names) = names {
        // Update specific plugins
        for name in plugin_names {
            if let Err(e) = rt.block_on(PluginInstaller::update(&name)) {
                eprintln!("Failed to update {}: {}", name, e);
            }
        }
    } else {
        // Update all plugins
        let plugins = list_plugins()?;
        if plugins.is_empty() {
            println!("No plugins installed.");
            return Ok(());
        }

        println!("Checking for updates...\n");
        for plugin in plugins {
            if let Err(e) = rt.block_on(PluginInstaller::update(&plugin.metadata.name)) {
                eprintln!("Failed to update {}: {}", plugin.metadata.name, e);
            }
        }
    }

    Ok(())
}

fn check_outdated_plugins() -> LpmResult<()> {
    let plugins = list_plugins()?;

    if plugins.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    println!("Checking for outdated plugins...\n");

    let rt = Runtime::new()
        .map_err(|e| LpmError::Package(format!("Failed to create runtime: {}", e)))?;
    let mut outdated_count = 0;

    for plugin in plugins {
        let current_version = plugin
            .installed_version
            .as_ref()
            .unwrap_or(&plugin.metadata.version);

        let registry = PluginRegistry::new();
        match rt.block_on(registry.get_latest_version(&plugin.metadata.name)) {
            Ok(Some(latest_version)) => {
                if current_version != &latest_version {
                    println!(
                        "  {}: {} -> {} (update available)",
                        plugin.metadata.name, current_version, latest_version
                    );
                    outdated_count += 1;
                } else {
                    println!(
                        "  {}: {} (up to date)",
                        plugin.metadata.name, current_version
                    );
                }
            }
            Ok(None) => {
                println!(
                    "  {}: {} (latest version unknown)",
                    plugin.metadata.name, current_version
                );
            }
            Err(e) => {
                println!(
                    "  {}: {} (error checking: {})",
                    plugin.metadata.name, current_version, e
                );
            }
        }
    }

    if outdated_count == 0 {
        println!("\nAll plugins are up to date.");
    } else {
        println!(
            "\n{} plugin(s) have updates available. Run 'lpm plugin update' to update them.",
            outdated_count
        );
    }

    Ok(())
}

fn search_plugins(query: Option<String>) -> LpmResult<()> {
    let search_query = query.as_deref().unwrap_or("lpm");
    println!("Searching for plugins matching '{}'...\n", search_query);

    let rt = Runtime::new()
        .map_err(|e| LpmError::Package(format!("Failed to create runtime: {}", e)))?;

    let registry = PluginRegistry::new();
    match rt.block_on(registry.search(search_query)) {
        Ok(results) => {
            if results.is_empty() {
                println!("No plugins found matching '{}'", search_query);
                println!("\nTry searching with: lpm plugin search <query>");
            } else {
                println!("Found {} plugin(s):\n", results.len());
                for entry in results {
                    println!("  {} ({})", entry.name, entry.version);
                    if let Some(desc) = &entry.description {
                        println!("    {}", desc);
                    }
                    if let Some(homepage) = &entry.homepage {
                        println!("    {}", homepage);
                    }
                    println!();
                }
                println!("Install with: lpm install -g lpm-<name>");
            }
        }
        Err(e) => {
            return Err(LpmError::Package(format!("Search failed: {}", e)));
        }
    }

    Ok(())
}
