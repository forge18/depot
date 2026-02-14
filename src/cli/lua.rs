use clap::Subcommand;
use depot::core::path::depot_home;
use depot::core::{DepotError, DepotResult};
use depot::lua_manager::{LuaDownloader, VersionSwitcher, WrapperGenerator};
use std::env;
use std::path::Path;
use std::process::Command;

#[derive(Subcommand)]
pub enum LuaCommands {
    /// List installed Lua versions
    #[command(aliases = ["ls"])]
    List,
    /// List available Lua versions for installation
    #[command(aliases = ["ls-remote"])]
    ListRemote,
    /// Install a Lua version
    Install {
        /// Version to install (e.g., "5.4.8", "latest", "5.4")
        version: String,
    },
    /// Switch to a Lua version globally
    Use {
        /// Version to use
        version: String,
    },
    /// Set Lua version for current project (creates .lua-version)
    Local {
        /// Version to use
        version: String,
    },
    /// Show currently active Lua version
    Current,
    /// Show which Lua version will be used (respects .lua-version)
    Which,
    /// Uninstall a Lua version
    Uninstall {
        /// Version to uninstall
        version: String,
    },
    /// Execute a command with a specific Lua version
    Exec {
        /// Lua version to use
        version: String,
        /// Command to execute
        command: Vec<String>,
    },
}

pub async fn run(command: LuaCommands) -> DepotResult<()> {
    let depot_home = depot_home()?;
    let cache_dir = depot::core::path::cache_dir()?;

    match command {
        LuaCommands::List => list_installed(&depot_home),
        LuaCommands::ListRemote => list_remote(&cache_dir),
        LuaCommands::Install { version } => install(&depot_home, &cache_dir, &version).await,
        LuaCommands::Use { version } => use_version(&depot_home, &version),
        LuaCommands::Local { version } => set_local(&depot_home, &version),
        LuaCommands::Current => show_current(&depot_home),
        LuaCommands::Which => show_which(&depot_home),
        LuaCommands::Uninstall { version } => uninstall(&depot_home, &version),
        LuaCommands::Exec { version, command } => exec(&depot_home, &version, command),
    }
}

fn list_installed(depot_home: &Path) -> DepotResult<()> {
    let switcher = VersionSwitcher::new(depot_home);
    let versions = switcher.list_installed()?;

    if versions.is_empty() {
        println!("No Lua versions installed.");
        println!("Install one with: depot lua install <version>");
        return Ok(());
    }

    let current = switcher.current().ok();
    for version in versions {
        if Some(&version) == current.as_ref() {
            println!("  {} (current)", version);
        } else {
            println!("  {}", version);
        }
    }

    Ok(())
}

fn list_remote(_cache_dir: &Path) -> DepotResult<()> {
    // For now, just show the known versions
    // In the future, we could fetch from GitHub releases API
    println!("Available Lua versions:");
    println!("  5.1.5");
    println!("  5.3.6");
    println!("  5.4.8");
    println!();
    println!("Note: Other versions may be available with custom sources.");
    println!("Configure with: depot config set lua_binary_sources.<version> <url>");
    Ok(())
}

async fn install(depot_home: &Path, cache_dir: &Path, version: &str) -> DepotResult<()> {
    let downloader = LuaDownloader::new(cache_dir.to_path_buf())?;
    let switcher = VersionSwitcher::new(depot_home);
    let resolved_version = downloader.resolve_version(version);

    // Check if already installed
    let installed = switcher.list_installed()?;
    if installed.contains(&resolved_version) {
        println!("Lua {} is already installed.", resolved_version);
        return Ok(());
    }

    println!("Installing Lua {}...", resolved_version);

    // Download binaries
    let lua_binary = downloader.download_binary(&resolved_version, "lua").await?;
    let luac_binary = downloader
        .download_binary(&resolved_version, "luac")
        .await?;

    // Install to versions directory
    let install_dir = depot_home.join("versions").join(&resolved_version);
    let bin_dir = install_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;

    std::fs::copy(&lua_binary, bin_dir.join("lua"))?;
    std::fs::copy(&luac_binary, bin_dir.join("luac"))?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(bin_dir.join("lua"))?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(bin_dir.join("lua"), perms)?;
        let mut perms = std::fs::metadata(bin_dir.join("luac"))?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(bin_dir.join("luac"), perms)?;
    }

    // Generate wrappers (if first install)
    let wrapper_gen = WrapperGenerator::new(depot_home);
    wrapper_gen.generate()?; // Always generate/re-generate to ensure they are up-to-date

    println!("✓ Successfully installed Lua {}", resolved_version);
    println!();
    println!("Use it with: depot lua use {}", resolved_version);

    // Auto-use if it's the first version installed
    if installed.is_empty() {
        switcher.switch(&resolved_version)?;
    }

    Ok(())
}

fn use_version(depot_home: &Path, version: &str) -> DepotResult<()> {
    let switcher = VersionSwitcher::new(depot_home);
    switcher.switch(version)
}

fn set_local(depot_home: &Path, version: &str) -> DepotResult<()> {
    let switcher = VersionSwitcher::new(depot_home);
    let current_dir = env::current_dir()?;
    switcher.set_local(version, &current_dir)
}

fn show_current(depot_home: &Path) -> DepotResult<()> {
    let switcher = VersionSwitcher::new(depot_home);
    let version = switcher.current()?;
    println!("{}", version);
    Ok(())
}

fn show_which(depot_home: &Path) -> DepotResult<()> {
    let current_dir = env::current_dir()?;
    let mut dir = current_dir.clone();
    let mut project_version = None;

    // Walk up directory tree looking for .lua-version
    loop {
        let version_file = dir.join(".lua-version");
        if version_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&version_file) {
                project_version = Some(content.trim().to_string());
                break;
            }
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    if let Some(ver) = project_version {
        println!("{} (from .lua-version)", ver);
    } else {
        let switcher = VersionSwitcher::new(depot_home);
        let version = switcher.current()?;
        println!("{} (global)", version);
    }

    Ok(())
}

fn uninstall(depot_home: &Path, version: &str) -> DepotResult<()> {
    let switcher = VersionSwitcher::new(depot_home);
    let version_dir = depot_home.join("versions").join(version);

    if !version_dir.exists() {
        return Err(DepotError::Package(format!(
            "Lua {} is not installed.",
            version
        )));
    }

    // Check if it's the current version
    if let Ok(current) = switcher.current() {
        if current == version {
            return Err(DepotError::Package(format!(
                "Cannot uninstall Lua {} because it is currently active.\n\
                 Switch to another version first: depot lua use <version>",
                version
            )));
        }
    }

    std::fs::remove_dir_all(&version_dir)?;
    println!("✓ Uninstalled Lua {}", version);
    Ok(())
}

fn exec(depot_home: &Path, version: &str, command: Vec<String>) -> DepotResult<()> {
    if command.is_empty() {
        return Err(DepotError::Package("No command provided".to_string()));
    }

    let version_dir = depot_home.join("versions").join(version);
    let lua_bin = version_dir.join("bin").join("lua");

    if !lua_bin.exists() {
        return Err(DepotError::Package(format!(
            "Lua {} is not installed. Run: depot lua install {}",
            version, version
        )));
    }

    let status = Command::new(&lua_bin).args(&command).status()?;
    let exit_code = status.code().unwrap_or(1);

    if exit_code != 0 {
        return Err(DepotError::SubprocessExit(exit_code));
    }
    Ok(())
}
