use clap::{Parser, Subcommand};
use lpm::core::LpmError;
use tracing_subscriber::EnvFilter;

mod cli;

#[derive(Parser)]
#[command(name = "lpm")]
#[command(about = "Local package management for Lua")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new LPM project
    Init {
        /// Use a template
        #[arg(short, long)]
        template: Option<String>,
        /// Skip interactive wizard (use defaults)
        #[arg(short, long)]
        yes: bool,
    },
    /// Create a new LPM project in a new directory
    New {
        /// Name of the project (creates directory)
        name: String,
        /// Template to use
        #[arg(short, long)]
        template: Option<String>,
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
    },
    /// Remove a dependency
    Remove {
        /// Package name to remove
        package: String,
        /// Remove global package
        #[arg(short = 'g', long)]
        global: bool,
    },
    /// Update dependencies
    Update {
        /// Package name to update (optional)
        package: Option<String>,
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
    /// Show outdated packages
    Outdated,
    /// Clean lua_modules directory
    Clean,
    /// Run a script
    Run {
        /// Script name
        script: String,
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
    },
    /// Publish package to LuaRocks
    Publish {
        /// Include pre-built Rust binaries
        #[arg(long)]
        with_binaries: bool,
    },
    /// Login to LuaRocks
    Login,
    /// Generate rockspec from package.yaml
    GenerateRockspec,
    /// Package built binaries
    Package {
        /// Target platform
        #[arg(short, long)]
        target: Option<String>,
    },
    /// Security audit
    Audit,
    /// Setup PATH for LPM (Unix only) - adds ~/.cargo/bin to PATH
    SetupPath,
    /// Manage Lua versions
    #[command(subcommand)]
    Lua(cli::lua::LuaCommands),
    /// Manage project templates
    #[command(subcommand)]
    Template(cli::template::TemplateCommands),
    /// Manage plugins
    #[command(subcommand)]
    Plugin(cli::plugin::commands::PluginSubcommand),
    /// External subcommands (plugins)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[tokio::main]
async fn main() -> Result<(), LpmError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Check PATH setup (only on first run, not for every command)
    // Skip for development builds (when running via cargo run)
    if !cfg!(debug_assertions) {
        let _ = lpm::core::path_setup::check_path_setup();
    }

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { template, yes } => cli::init::run(template, yes).await,
        Commands::New {
            name,
            template,
            yes,
        } => cli::new::run(name, template, yes).await,
        Commands::Install {
            package,
            dev,
            path,
            no_dev,
            dev_only,
            global,
            interactive,
        } => cli::install::run(package, dev, path, no_dev, dev_only, global, interactive).await,
        Commands::Remove { package, global } => cli::remove::run(package, global),
        Commands::Update { package } => cli::update::run(package).await,
        Commands::List { tree, global } => cli::list::run(tree, global),
        Commands::Verify => cli::verify::run(),
        Commands::Outdated => cli::outdated::run().await,
        Commands::Clean => cli::clean::run(),
        Commands::Run { script } => cli::run::run(script),
        Commands::Exec { command } => cli::exec::run(command),
        Commands::Build {
            target,
            all_targets,
        } => cli::build::run(target, all_targets),
        Commands::Package { target } => cli::package::run(target),
        Commands::Publish { with_binaries } => cli::publish::run(with_binaries).await,
        Commands::Login => cli::login::run().await,
        Commands::GenerateRockspec => cli::generate_rockspec::run(),
        Commands::Audit => cli::audit::run().await,
        Commands::SetupPath => {
            lpm::core::path_setup::setup_path_auto()?;
            Ok(())
        }
        Commands::Lua(cmd) => cli::lua::run(cmd).await,
        Commands::Template(cmd) => cli::template::run(cmd),
        Commands::Plugin(cmd) => cli::plugin::commands::run(cmd),
        Commands::External(args) => {
            if args.is_empty() {
                return Err(LpmError::Package("Command required".to_string()));
            }
            cli::plugin::run_plugin(&args[0], args[1..].to_vec())
        }
    };

    // Display error with helpful suggestions
    if let Err(ref e) = result {
        eprintln!("\n{}", lpm::core::error_help::format_error_with_help(e));
    }

    result
}
