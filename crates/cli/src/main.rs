//! CLI entry point for the Stacks native indexer.

mod codegen;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use stacks_indexer_core::config;
use stacks_indexer_storage::Database;

#[derive(Parser)]
#[command(
    name = "stacks-indexer",
    version,
    about = "High-performance native Stacks blockchain indexer"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the indexer in production mode.
    Start {
        /// Path to config file.
        #[arg(short, long, default_value = "stacks-indexer.yaml")]
        config: PathBuf,
    },
    /// Start in development mode with verbose logging and GraphQL playground.
    Dev {
        /// Path to config file.
        #[arg(short, long, default_value = "stacks-indexer.yaml")]
        config: PathBuf,
    },
    /// Show sync status (last block, table row counts).
    Status {
        /// Path to config file.
        #[arg(short, long, default_value = "stacks-indexer.yaml")]
        config: PathBuf,
    },
    /// Reset database and re-index from scratch.
    Reset {
        /// Path to config file.
        #[arg(short, long, default_value = "stacks-indexer.yaml")]
        config: PathBuf,
    },
    /// Scaffold a new indexer project directory.
    Init {
        /// Project name (becomes directory name).
        name: String,
    },
    /// Generate Rust types from the YAML config.
    Codegen {
        /// Path to config file.
        #[arg(short, long, default_value = "stacks-indexer.yaml")]
        config: PathBuf,
        /// Output file path.
        #[arg(short, long, default_value = "src/generated.rs")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config: path } => {
            init_tracing("info");
            let cfg = config::load_config(&path)?;
            stacks_indexer_server::run(cfg, false).await?;
        }
        Commands::Dev { config: path } => {
            init_tracing("debug");
            let cfg = config::load_config(&path)?;
            tracing::info!("starting in dev mode with verbose logging");
            stacks_indexer_server::run(cfg, true).await?;
        }
        Commands::Status { config: path } => {
            let cfg = config::load_config(&path)?;
            print_status(&cfg)?;
        }
        Commands::Reset { config: path } => {
            let cfg = config::load_config(&path)?;
            reset_db(&cfg)?;
        }
        Commands::Init { name } => {
            scaffold_project(&name)?;
        }
        Commands::Codegen { config: path, output } => {
            init_tracing("info");
            let cfg = config::load_config(&path)?;
            codegen::generate(&cfg, &output)?;
        }
    }

    Ok(())
}

fn init_tracing(default_level: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn print_status(cfg: &config::IndexerConfig) -> anyhow::Result<()> {
    let db = Database::open(cfg)?;
    let (height, hash) = db.get_last_processed_block()?;

    println!("Indexer Status");
    println!("  Database:          {}", cfg.storage.path);
    println!("  Last block height: {height}");
    println!(
        "  Last block hash:   {}",
        if hash.is_empty() { "(none)" } else { &hash }
    );

    for (table, count) in db.table_row_counts(cfg)? {
        println!("  Table {table}: {count} rows");
    }

    Ok(())
}

fn reset_db(cfg: &config::IndexerConfig) -> anyhow::Result<()> {
    let path = &cfg.storage.path;
    if std::path::Path::new(path).exists() {
        std::fs::remove_file(path)?;
        let _ = std::fs::remove_file(format!("{path}-wal"));
        let _ = std::fs::remove_file(format!("{path}-shm"));
        println!("Database reset: {path}");
    } else {
        println!("No database found at {path}");
    }
    Ok(())
}

fn scaffold_project(name: &str) -> anyhow::Result<()> {
    let dir = std::path::Path::new(name);
    std::fs::create_dir_all(dir)?;

    std::fs::write(
        dir.join("stacks-indexer.yaml"),
        format!(
            r#"name: "{name}"
network: devnet
server:
  event_listener_port: 20445
  api_port: 4000
storage:
  path: "./data/indexer.db"
sources:
  - contract: "ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM.my-contract"
    start_block: 0
    events:
      - name: my_event
        type: print_event
        table: my_events
"#
        ),
    )?;

    println!("Scaffolded project '{name}'");
    println!("  Created {name}/stacks-indexer.yaml");
    println!();
    println!("Next steps:");
    println!("  1. Edit stacks-indexer.yaml with your contracts and events");
    println!("  2. Run: stacks-indexer dev -c {name}/stacks-indexer.yaml");
    Ok(())
}
