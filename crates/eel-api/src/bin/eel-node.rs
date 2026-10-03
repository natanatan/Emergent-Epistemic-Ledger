use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// EEL node: serves the ledger API over HTTP.
#[derive(Parser)]
#[command(name = "eel-node", version)]
struct Cli {
    /// Data directory (default: $EEL_HOME or ./.eel).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the node.
    Start {
        #[arg(long, default_value = "127.0.0.1:7878")]
        listen: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cli = Cli::parse();
    let dir = cli
        .data_dir
        .or_else(|| std::env::var_os("EEL_HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".eel"));
    match cli.command {
        Command::Start { listen } => eel_api::serve(&dir, &listen).await,
    }
}
