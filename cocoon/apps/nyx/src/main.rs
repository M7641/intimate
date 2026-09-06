mod agents;
mod llm;
mod model;
mod tools;
pub mod ui;

use std::sync::Arc;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nyx", about = "CLI agents powered by Claude")]
pub struct Cli {
    /// Use local TinyLlama model instead of Claude
    #[arg(long)]
    local: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a commit message from staged changes and commit
    Commit,
    /// Ask a question to the LLM
    Muse {
        /// The question to ask
        question: Vec<String>,
    },
    /// Scaffold a documentation site and generate initial content
    Docs {
        /// Project title (auto-detected if omitted)
        #[arg(long, short)]
        title: Option<String>,
    },
    /// Scan files for code that needs more explanation
    Clarity {
        /// Save report to a timestamped markdown file
        #[arg(long, short)]
        save: bool,

        /// Paths to scan (interactive picker if omitted)
        files: Vec<String>,
    },
    /// Run a chain of impeccable.style design skills (audit → polish → critique)
    Impeccable {
        /// Skip confirmation between steps and run the full chain
        #[arg(long)]
        auto: bool,

        /// Save the chained outputs to a timestamped markdown report
        #[arg(long, short)]
        save: bool,

        /// Comma-separated chain of skills (default: audit,polish,critique)
        #[arg(long)]
        chain: Option<String>,

        /// Files to scope the chain to (interactive picker if omitted)
        files: Vec<String>,
    },
    /// Generate a TSV of business deliverables for an exec audience
    Scope {
        /// Directory to scan (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,

        /// Output TSV path (defaults to DELIVERABLES.tsv in the cwd)
        #[arg(long, short, default_value = "DELIVERABLES.tsv")]
        output: String,
    },
    /// Set up shell completions (auto-detects your shell)
    Init,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tools::shell_init::refresh_if_needed();

    if let Commands::Init = cli.command {
        ui::banner();
        tools::shell_init::run()?;
        return Ok(());
    }

    ui::banner();
    let llm: Arc<dyn llm::LlmClient> = Arc::from(llm::auto_client(cli.local)?);

    match cli.command {
        Commands::Commit => agents::commit::run(&*llm).await?,
        Commands::Muse { question } => {
            let question = question.join(" ");
            agents::muse::run(&*llm, &question).await?;
        }
        Commands::Docs { title } => {
            agents::docs::run(Arc::clone(&llm), title).await?;
        }
        Commands::Clarity { save, files } => {
            let paths = if files.is_empty() {
                tools::picker::pick_files_with_limit(5)?
            } else {
                files
            };
            let parallel = !cli.local;
            agents::clarity::run(Arc::clone(&llm), &paths, parallel, save).await?;
        }
        Commands::Impeccable {
            auto,
            save,
            chain,
            files,
        } => {
            let paths = if files.is_empty() {
                tools::picker::pick_files_with_limit(0)?
            } else {
                files
            };
            agents::impeccable::run(&paths, chain, auto, save).await?;
        }
        Commands::Scope { path, output } => {
            agents::scope::run(Arc::clone(&llm), path.into(), output.into()).await?;
        }
        Commands::Init => unreachable!(),
    }

    Ok(())
}
