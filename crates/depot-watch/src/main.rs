use clap::{Parser, Subcommand};
use depot_core::DepotError;

mod ui;
mod watch;
mod websocket;

#[derive(Parser)]
#[command(name = "depot-watch")]
#[command(about = "Watch files and restart on changes")]
struct Cli {
    #[command(subcommand)]
    command: WatchCommands,
}

#[derive(Subcommand)]
enum WatchCommands {
    /// Watch files and restart on changes
    Watch {
        /// Command to run (e.g., "lua src/main.lua")
        #[arg(short, long)]
        command: Option<Vec<String>>,

        /// Paths to watch (default: src/, lib/)
        #[arg(short, long)]
        paths: Option<Vec<String>>,

        /// Patterns to ignore (e.g., "**/*.swp")
        #[arg(short, long)]
        ignore: Option<Vec<String>>,

        /// Don't clear screen on restart
        #[arg(long)]
        no_clear: bool,

        /// Script name from package.yaml to run
        #[arg(short, long)]
        script: Option<String>,

        /// WebSocket port for browser reload (0 = disabled)
        #[arg(long, default_value = "0")]
        websocket_port: u16,
    },

    /// Start dev server (alias for watch)
    Dev {
        /// Script name from package.yaml
        script: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), DepotError> {
    let cli = Cli::parse();

    match cli.command {
        WatchCommands::Watch {
            command,
            paths,
            ignore,
            no_clear,
            script,
            websocket_port,
        } => {
            watch::run(
                command,
                paths,
                ignore,
                no_clear,
                script,
                Some(websocket_port),
            )
            .await
        }
        WatchCommands::Dev { script } => watch::run(None, None, None, false, script, None).await,
    }
}
