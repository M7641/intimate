use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "nameless",
    about = "Reinforcement gymnasium for Rust code optimization"
)]
struct Cli {
    /// Use local TinyLlama model instead of Claude
    #[arg(long)]
    local: bool,

    /// Disable TUI dashboard, print progress to stderr
    #[arg(long)]
    no_tui: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a training loop on a challenge
    Run {
        /// Path to the challenge directory
        path: String,
    },
    /// Scaffold a new challenge
    New {
        /// Name for the new challenge
        name: String,
    },
    /// List all challenges with best scores
    List,
    /// Show leaderboard for a challenge
    Show {
        /// Challenge name
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    nameless::ui::banner();

    match cli.command {
        Commands::Run { path } => {
            let pool = nameless::db::init_pool("nameless.db").await?;
            let llm: Arc<dyn nameless::llm::LlmClient> =
                Arc::from(nameless::llm::auto_client(cli.local)?);
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

            if cli.no_tui {
                let printer = tokio::spawn(nameless::ui::print_events(rx));
                nameless::gym::run(PathBuf::from(path), llm, pool, tx).await?;
                printer.await?;
            } else {
                let path = PathBuf::from(path);
                let gym_handle = tokio::spawn(nameless::gym::run(path, llm, pool, tx));
                nameless::ui::widgets::dashboard::run(rx).await?;
                gym_handle.await??;
            }
        }
        Commands::New { name } => {
            let dir = PathBuf::from("challenges").join(&name);
            if dir.exists() {
                anyhow::bail!("challenge directory already exists: {}", dir.display());
            }
            std::fs::create_dir_all(&dir)?;

            let toml_content = format!(
                r#"name = "{name}"
description = "TODO: describe the challenge"
signature = "fn solve(input: &[i64]) -> i64"

[config]
max_generations = 10
candidates_per_generation = 3
timeout_compile_secs = 30
timeout_run_secs = 5

[[tests]]
input = "&[1, 2, 3]"
expected = "6"
"#
            );
            std::fs::write(dir.join("challenge.toml"), toml_content)?;
            std::fs::write(
                dir.join("reference.rs"),
                "fn solve(input: &[i64]) -> i64 {\n    input.iter().sum()\n}\n",
            )?;

            nameless::ui::status(&format!("created challenge scaffold at {}", dir.display()));
        }
        Commands::List => {
            let db_path = "nameless.db";
            if !std::path::Path::new(db_path).exists() {
                nameless::ui::status("no results yet (run a challenge first)");
                return Ok(());
            }
            let pool = nameless::db::init_pool(db_path).await?;
            let rows = nameless::db::list_challenges_with_scores(&pool).await?;
            if rows.is_empty() {
                nameless::ui::status("no challenges found");
            } else {
                nameless::ui::leaderboard_summary(&rows);
            }
        }
        Commands::Show { name } => {
            let db_path = "nameless.db";
            if !std::path::Path::new(db_path).exists() {
                nameless::ui::status("no results yet (run a challenge first)");
                return Ok(());
            }
            let pool = nameless::db::init_pool(db_path).await?;
            let rows = nameless::db::get_leaderboard(&pool, &name).await?;
            if rows.is_empty() {
                nameless::ui::status(&format!("no results for challenge '{name}'"));
            } else {
                nameless::ui::full_leaderboard(&name, &rows);
            }
        }
    }

    Ok(())
}
