use clap::{Parser, Subcommand};
use depot_core::DepotError;

mod bundle;
mod bundler;

#[derive(Parser)]
#[command(name = "depot-bundle")]
#[command(about = "Bundle Lua files into a single file (experimental)")]
struct Cli {
    #[command(subcommand)]
    command: BundleCommands,
}

#[derive(Subcommand)]
enum BundleCommands {
    /// Bundle Lua files into a single file
    Bundle {
        /// Entry point file (default: src/main.lua)
        #[arg(short, long)]
        entry: Option<String>,

        /// Output file (default: dist/bundle.lua)
        #[arg(short, long)]
        output: Option<String>,

        /// Minify output (strip whitespace, comments)
        #[arg(short, long)]
        minify: bool,

        /// Generate source map
        #[arg(short, long)]
        source_map: bool,

        /// Strip comments (but don't minify)
        #[arg(long)]
        no_comments: bool,

        /// Enable tree-shaking (remove unused code)
        #[arg(long)]
        tree_shake: bool,

        /// Track dynamic requires (runtime analysis)
        #[arg(long)]
        dynamic_requires: bool,

        /// Incremental bundling (only rebuild changed modules)
        #[arg(long)]
        incremental: bool,
    },
    /// Watch mode: automatically rebundle on file changes
    Watch {
        /// Entry point file (default: src/main.lua)
        #[arg(short, long)]
        entry: Option<String>,

        /// Output file (default: dist/bundle.lua)
        #[arg(short, long)]
        output: Option<String>,

        /// Minify output
        #[arg(short, long)]
        minify: bool,

        /// Generate source map
        #[arg(short, long)]
        source_map: bool,

        /// Enable tree-shaking
        #[arg(long)]
        tree_shake: bool,
    },
}

fn main() -> Result<(), DepotError> {
    // Show experimental warning
    eprintln!("⚠️  depot-bundle is EXPERIMENTAL");
    eprintln!("   Static analysis has limitations:");
    eprintln!("   - Dynamic requires (require(variable)) are not detected");
    eprintln!("   - C modules cannot be bundled");
    eprintln!("   - Minifier is basic and may not work for all code");
    eprintln!();

    let cli = Cli::parse();

    match cli.command {
        BundleCommands::Bundle {
            entry,
            output,
            minify,
            source_map,
            no_comments,
            tree_shake,
            dynamic_requires,
            incremental,
        } => {
            use crate::bundle::BundleRunOptions;
            let opts = BundleRunOptions {
                entry,
                output,
                minify,
                source_map,
                no_comments,
                tree_shake,
                dynamic_requires,
                incremental,
            };
            bundle::run_with_options(opts)
        }
        BundleCommands::Watch {
            entry,
            output,
            minify,
            source_map,
            tree_shake,
        } => bundle::run_watch(entry, output, minify, source_map, tree_shake),
    }
}
