use clap::{Parser, Subcommand};
use depot::core::DepotError;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

mod cli;

#[derive(Parser)]
#[command(name = "depot")]
#[command(about = "Local package management for Lua")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Depot project
    Init {
        /// Skip interactive wizard (use defaults)
        #[arg(short, long)]
        yes: bool,
    },
    /// Create a new Depot project in a new directory
    New {
        /// Name of the project (creates directory)
        name: String,
        /// Skip interactive prompts
        #[arg(short, long)]
        yes: bool,
    },
    /// Install dependencies
    Install {
        /// Package name to install
        package: Option<String>,
        /// Install as dev dependency
        #[arg(short, long)]
        dev: bool,
        /// Install from local path
        #[arg(short, long)]
        path: Option<String>,
        /// Skip dev dependencies (production install)
        #[arg(long)]
        no_dev: bool,
        /// Install only dev dependencies
        #[arg(long)]
        dev_only: bool,
        /// Install globally (like npm install -g)
        #[arg(short = 'g', long)]
        global: bool,
        /// Interactive mode: search and select packages
        #[arg(short, long)]
        interactive: bool,
        /// Filter workspace packages (e.g., "package-a", "packages/*", "...package-a", "package-a...")
        #[arg(short = 'f', long)]
        filter: Vec<String>,
        /// Install from a specific branch
        #[arg(short = 'b', long, conflicts_with_all = &["commit", "release"])]
        branch: Option<String>,
        /// Install from a specific commit SHA
        #[arg(short = 'c', long, conflicts_with_all = &["branch", "release"])]
        commit: Option<String>,
        /// Install from a specific release tag
        #[arg(short = 'r', long, conflicts_with_all = &["branch", "commit"])]
        release: Option<String>,
    },
    /// Remove a dependency
    Remove {
        /// Package name to remove
        package: String,
        /// Remove global package
        #[arg(short = 'g', long)]
        global: bool,
        /// Filter workspace packages (e.g., "package-a", "packages/*", "...package-a", "package-a...")
        #[arg(short = 'f', long)]
        filter: Vec<String>,
    },
    /// Update dependencies
    Update {
        /// Package name to update (optional)
        package: Option<String>,
        /// Filter workspace packages (e.g., "package-a", "packages/*", "...package-a", "package-a...")
        #[arg(short = 'f', long)]
        filter: Vec<String>,
    },
    /// List installed packages
    List {
        /// Show dependency tree
        #[arg(short, long)]
        tree: bool,
        /// List global packages
        #[arg(short = 'g', long)]
        global: bool,
    },
    /// Verify package checksums
    Verify,
    /// Clean lua_modules directory
    Clean,
    /// Run a script
    Run {
        /// Script name
        script: String,
        /// Filter workspace packages (e.g., "package-a", "packages/*", "...package-a", "package-a...")
        #[arg(short = 'f', long)]
        filter: Vec<String>,
    },
    /// Execute a command with correct paths
    Exec {
        /// Command to execute
        command: Vec<String>,
    },
    /// Build Rust extensions
    Build {
        /// Target platform
        #[arg(short, long)]
        target: Option<String>,
        /// Build for all common targets
        #[arg(long)]
        all_targets: bool,
        /// Filter workspace packages (e.g., "package-a", "packages/*", "...package-a", "package-a...")
        #[arg(short = 'f', long)]
        filter: Vec<String>,
    },
    /// Package built binaries
    Package {
        /// Target platform
        #[arg(short, long)]
        target: Option<String>,
    },
    /// Security audit
    Audit,
    /// Check Lua version compatibility
    Compat {
        /// Only show incompatibilities
        #[arg(short, long)]
        quiet: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Setup Depot environment (install to DEPOT_HOME and configure PATH)
    Setup,
    /// Configure global settings
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Manage Lua versions
    #[command(subcommand)]
    Lua(cli::lua::LuaCommands),
    /// Manage plugins
    #[command(subcommand)]
    Plugin(cli::plugin::commands::PluginSubcommand),
    /// Workspace commands
    #[command(subcommand)]
    Workspace(WorkspaceCommands),
    /// External subcommands (plugins)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Set global installation directory
    SetGlobalPath {
        /// Path to global installation directory
        path: String,
    },
    /// Show current global installation path
    GetGlobalPath,
}

#[derive(Subcommand)]
enum WorkspaceCommands {
    /// List all packages in the workspace
    List,
    /// Show detailed workspace information
    Info,
    /// Show shared dependencies across workspace packages
    SharedDeps,
}

fn set_global_path(path: String) -> depot::core::DepotResult<()> {
    use depot::config::Config;
    use std::fs;
    use std::path::PathBuf;

    let path_buf = PathBuf::from(&path);

    // Validate that the path exists or can be created
    if !path_buf.exists() {
        fs::create_dir_all(&path_buf)
            .map_err(|e| DepotError::Path(format!("Failed to create directory: {}", e)))?;
    }

    // Load or create config
    let mut config = Config::load().unwrap_or_default();

    // Set global path
    config.global_install_path = Some(path_buf.clone());

    // Save config
    config.save()?;

    println!("✓ Global installation path set to: {}", path_buf.display());

    Ok(())
}

fn get_global_path() -> depot::core::DepotResult<()> {
    use depot::config::Config;
    use depot::core::path::global_dir;

    let config = Config::load().unwrap_or_default();

    let global_path = if let Some(ref custom_path) = config.global_install_path {
        custom_path.clone()
    } else {
        global_dir()?
    };

    println!("Global installation path: {}", global_path.display());

    if config.global_install_path.is_some() {
        println!("  (custom path set in config)");
    } else {
        println!("  (default system path)");
    }

    Ok(())
}

/// Setup Depot environment (pnpm-style setup)
///
/// This command:
/// 1. Creates DEPOT_HOME directory (~/.local/share/depot)
/// 2. Copies the current depot executable to DEPOT_HOME/bin
/// 3. Adds DEPOT_HOME/bin to PATH by updating shell config
/// 4. Sets up the environment for immediate use
fn depot_setup() -> depot::core::DepotResult<()> {
    use std::env;
    use std::fs;

    println!("🔧 Setting up Depot environment...\n");

    // 1. Determine DEPOT_HOME
    let depot_home = get_depot_home()?;
    let depot_bin = depot_home.join("bin");

    println!("  DEPOT_HOME: {}", depot_home.display());

    // 2. Create directories
    fs::create_dir_all(&depot_bin)
        .map_err(|e| DepotError::Path(format!("Failed to create DEPOT_HOME/bin: {}", e)))?;

    // 3. Get current executable path
    let current_exe = env::current_exe()
        .map_err(|e| DepotError::Path(format!("Failed to get current executable: {}", e)))?;

    let exe_name = if cfg!(target_os = "windows") {
        "depot.exe"
    } else {
        "depot"
    };

    let target_exe = depot_bin.join(exe_name);

    // 4. Copy executable to DEPOT_HOME/bin (if not already there)
    if current_exe != target_exe {
        println!("  Copying depot executable to DEPOT_HOME/bin...");
        fs::copy(&current_exe, &target_exe)
            .map_err(|e| DepotError::Path(format!("Failed to copy executable: {}", e)))?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&target_exe)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&target_exe, perms)?;
        }

        println!("  ✓ Executable installed to: {}", target_exe.display());
    } else {
        println!("  ✓ Executable already in DEPOT_HOME/bin");
    }

    // 5. Configure PATH
    if cfg!(target_os = "windows") {
        setup_path_windows(&depot_bin)?;
    } else {
        setup_path_unix(&depot_bin)?;
    }

    println!("\n✅ Depot setup complete!");
    println!("\nNext steps:");
    if cfg!(target_os = "windows") {
        println!("  1. Restart your terminal");
        println!("  2. Run: depot --version");
    } else {
        let shell = detect_shell();
        let profile = get_shell_profile(&shell);
        println!("  1. Reload your shell: source {}", profile);
        println!("  2. Run: depot --version");
    }

    Ok(())
}

/// Get DEPOT_HOME directory (like PNPM_HOME)
fn get_depot_home() -> depot::core::DepotResult<std::path::PathBuf> {
    use std::env;
    use std::path::PathBuf;

    // Check DEPOT_HOME environment variable first
    if let Ok(depot_home) = env::var("DEPOT_HOME") {
        return Ok(PathBuf::from(depot_home));
    }

    // Default locations (following XDG on Linux, standard on macOS/Windows)
    if cfg!(target_os = "windows") {
        // Windows: %LOCALAPPDATA%\depot
        let local_app_data = env::var("LOCALAPPDATA")
            .map_err(|_| DepotError::Path("LOCALAPPDATA not set".to_string()))?;
        Ok(PathBuf::from(local_app_data).join("depot"))
    } else if cfg!(target_os = "macos") {
        // macOS: ~/Library/Application Support/depot
        let home = env::var("HOME").map_err(|_| DepotError::Path("HOME not set".to_string()))?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("depot"))
    } else {
        // Linux/Unix: ~/.local/share/depot (XDG Base Directory)
        let home = env::var("HOME").map_err(|_| DepotError::Path("HOME not set".to_string()))?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("depot"))
    }
}

/// Setup PATH on Windows
fn setup_path_windows(depot_bin: &std::path::Path) -> depot::core::DepotResult<()> {
    use std::process::Command;

    println!("\n  Configuring Windows PATH...");

    // Get current user PATH
    let output = Command::new("powershell")
        .args([
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path', 'User')",
        ])
        .output()
        .map_err(|e| DepotError::Path(format!("Failed to get PATH: {}", e)))?;

    let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let depot_bin_str = depot_bin.to_string_lossy();

    // Check if already in PATH
    if current_path
        .split(';')
        .any(|p| p.trim() == depot_bin_str.as_ref())
    {
        println!("  ✓ DEPOT_HOME/bin already in PATH");
        return Ok(());
    }

    // Add to PATH
    let new_path = if current_path.is_empty() {
        depot_bin_str.to_string()
    } else {
        format!("{};{}", depot_bin_str, current_path)
    };

    let result = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "[Environment]::SetEnvironmentVariable('Path', '{}', 'User')",
                new_path
            ),
        ])
        .output()
        .map_err(|e| DepotError::Path(format!("Failed to set PATH: {}", e)))?;

    if result.status.success() {
        println!("  ✓ Added DEPOT_HOME/bin to PATH");
        Ok(())
    } else {
        Err(DepotError::Path(format!(
            "Failed to update PATH: {}",
            String::from_utf8_lossy(&result.stderr)
        )))
    }
}

/// Setup PATH on Unix (Linux/macOS)
fn setup_path_unix(depot_bin: &std::path::Path) -> depot::core::DepotResult<()> {
    use std::env;
    use std::fs;
    use std::io::Write;

    println!("\n  Configuring shell PATH...");

    let shell = detect_shell();
    let profile_file = get_shell_profile(&shell);
    let home = env::var("HOME").map_err(|_| DepotError::Path("HOME not set".to_string()))?;
    let profile_path = profile_file.replace("~", &home);

    // Read current profile
    let profile_content = fs::read_to_string(&profile_path).unwrap_or_else(|_| String::new());

    // Check if DEPOT_HOME is already configured
    if profile_content.contains("DEPOT_HOME")
        || profile_content.contains(&depot_bin.to_string_lossy().to_string())
    {
        println!("  ✓ PATH already configured in {}", profile_file);
        return Ok(());
    }

    // Add DEPOT_HOME configuration
    let depot_home = depot_bin.parent().unwrap();
    let config_lines = format!(
        "\n# Depot environment\nexport DEPOT_HOME=\"{}\"\nexport PATH=\"$DEPOT_HOME/bin:$PATH\"\n",
        depot_home.display()
    );

    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&profile_path)
        .map_err(|e| DepotError::Path(format!("Failed to open {}: {}", profile_file, e)))?;

    file.write_all(config_lines.as_bytes())
        .map_err(|e| DepotError::Path(format!("Failed to write to {}: {}", profile_file, e)))?;

    println!("  ✓ Added DEPOT_HOME to {}", profile_file);
    println!("  ✓ Added DEPOT_HOME/bin to PATH");

    Ok(())
}

/// Detect the current shell
fn detect_shell() -> String {
    use std::env;
    env::var("SHELL")
        .unwrap_or_else(|_| "/bin/sh".to_string())
        .rsplit('/')
        .next()
        .unwrap_or("sh")
        .to_string()
}

/// Get the shell profile file path
fn get_shell_profile(shell: &str) -> String {
    match shell {
        "zsh" => "~/.zshrc".to_string(),
        "bash" => "~/.bashrc".to_string(),
        "fish" => "~/.config/fish/config.fish".to_string(),
        _ => "~/.profile".to_string(),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Check PATH setup (only on first run, not for every command)
    // Skip for development builds (when running via cargo run)
    if !cfg!(debug_assertions) {
        let _ = depot::core::path_setup::check_path_setup();
    }

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { yes } => cli::init::run(yes).await,
        Commands::New { name, yes } => cli::new::run(name, yes).await,
        Commands::Install {
            package,
            dev,
            path,
            no_dev,
            dev_only,
            global,
            interactive,
            filter,
            branch,
            commit,
            release,
        } => {
            cli::install::run(cli::install::InstallOptions {
                package,
                dev,
                path,
                no_dev,
                dev_only,
                global,
                interactive,
                filter,
                branch,
                commit,
                release,
            })
            .await
        }
        Commands::Remove {
            package,
            global,
            filter,
        } => cli::remove::run(package, global, filter),
        Commands::Update { package, filter } => cli::update::run(package, filter).await,
        Commands::List { tree, global } => cli::list::run(tree, global),
        Commands::Verify => cli::verify::run(),
        Commands::Clean => cli::clean::run(),
        Commands::Run { script, filter } => cli::run::run(script, filter),
        Commands::Exec { command } => cli::exec::run(command),
        Commands::Build {
            target,
            all_targets,
            filter,
        } => cli::build::run(target, all_targets, filter),
        Commands::Package { target } => cli::package::run(target),
        Commands::Audit => cli::audit::run().await,
        Commands::Compat { quiet, json } => cli::compat::run(quiet, json),
        Commands::Setup => depot_setup(),
        Commands::Config(cmd) => match cmd {
            ConfigCommands::SetGlobalPath { path } => set_global_path(path),
            ConfigCommands::GetGlobalPath => get_global_path(),
        },
        Commands::Lua(cmd) => cli::lua::run(cmd).await,
        Commands::Plugin(cmd) => cli::plugin::commands::run(cmd),
        Commands::Workspace(cmd) => match cmd {
            WorkspaceCommands::List => cli::workspace::list().await,
            WorkspaceCommands::Info => cli::workspace::info().await,
            WorkspaceCommands::SharedDeps => cli::workspace::shared_deps().await,
        },
        Commands::External(args) => {
            if args.is_empty() {
                Err(DepotError::Package("Command required".to_string()))
            } else {
                cli::plugin::run_plugin(&args[0], args[1..].to_vec())
            }
        }
    };

    // Handle result and exit codes
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            match &e {
                DepotError::SubprocessExit(code) => {
                    // Subprocess failed - exit with its code
                    // No error message (subprocess already printed it)
                    ExitCode::from(*code as u8)
                }
                DepotError::AuditFailed(_) => {
                    // Print audit error and exit with code 1
                    eprintln!("\n{}", depot::core::error_help::format_error_with_help(&e));
                    ExitCode::FAILURE
                }
                _ => {
                    // Regular Depot error - display with helpful suggestions
                    eprintln!("\n{}", depot::core::error_help::format_error_with_help(&e));
                    ExitCode::FAILURE
                }
            }
        }
    }
}
