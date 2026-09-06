//! forage — point it at a URL with no prior knowledge; it crawls, exercises the
//! features it finds, watches the API traffic, and reports what's broken.
//! Time-boxed to favour breadth of path coverage. Not a load tester.

mod browser;
mod budget;
mod bundle;
mod capture;
mod explore;
mod findings;
mod frontier;
mod models;
mod report;
mod safety;
mod scheduler;
mod validate;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Local;
use clap::Parser;
use tracing::info;

use crate::budget::Budget;
use crate::capture::console::ConsoleCapture;
use crate::capture::network::NetworkCapture;
use crate::findings::Severity;
use crate::frontier::route_key;
use crate::models::RunMeta;
use crate::scheduler::Scheduler;

#[derive(Parser)]
#[command(
    name = "forage",
    about = "Crawl an unknown site, exercise its features, and report what's broken"
)]
struct Cli {
    /// Base URL to explore.
    base_url: String,
    /// Global time box in seconds (~10 min default).
    #[arg(long, default_value_t = 600)]
    budget: u64,
    /// Output root for the run bundle.
    #[arg(long, default_value = "out")]
    out: PathBuf,
    /// Cap on pages visited.
    #[arg(long, default_value_t = 500)]
    max_pages: usize,
    /// Max link depth to follow.
    #[arg(long, default_value_t = 6)]
    max_depth: usize,
    /// Cap on query permutations per path.
    #[arg(long, default_value_t = 5)]
    max_per_route: usize,
    /// Min gap between navigations, ms (politeness).
    #[arg(long, default_value_t = 250)]
    rate_limit_ms: u64,
    /// Per-task hard cap, seconds.
    #[arg(long, default_value_t = 20)]
    action_timeout: u64,
    /// Disable exercising controls (crawl + observe only).
    #[arg(long)]
    no_exercise: bool,
    /// Allow submitting safe (non-destructive) POST forms.
    #[arg(long)]
    submit_forms: bool,
    /// Discover and classify controls but act on nothing.
    #[arg(long)]
    dry_run: bool,
    /// Show the browser window (debug).
    #[arg(long)]
    headful: bool,
}

/// Everything the scheduler needs from the CLI.
pub struct RunConfig {
    pub base_url: String,
    pub max_pages: usize,
    pub max_depth: usize,
    pub max_per_route: usize,
    pub exercise: bool,
    pub submit_forms: bool,
    pub dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "forage=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let had_errors = run(cli).await?;
    if had_errors {
        std::process::exit(1);
    }
    Ok(())
}

/// Returns `true` if any error-severity finding was recorded.
async fn run(cli: Cli) -> Result<bool> {
    let started = Instant::now();
    let started_at = Local::now().to_rfc3339();

    let cfg = RunConfig {
        base_url: cli.base_url.clone(),
        max_pages: cli.max_pages,
        max_depth: cli.max_depth,
        max_per_route: cli.max_per_route,
        exercise: !cli.no_exercise,
        submit_forms: cli.submit_forms,
        dry_run: cli.dry_run,
    };

    info!(url = %cli.base_url, budget = cli.budget, "launching browser");
    let session = browser::launch(cli.headful).await?;

    // Open blank first so listeners attach before the target's first request.
    let page = session.browser.new_page("about:blank").await?;
    let net = NetworkCapture::start(&page).await?;
    let console = ConsoleCapture::start(&page).await?;

    let budget = Budget::new(
        Duration::from_secs(cli.budget),
        Duration::from_millis(cli.rate_limit_ms),
        Duration::from_secs(cli.action_timeout),
    );
    let bundle = bundle::new_bundle(&cli.out)?;

    let scheduler = Scheduler::new(&page, &net, &console, &budget, &bundle, &cfg)?;
    let result = scheduler.run().await;

    net.stop();
    console.stop();
    session.close().await?;

    // Assemble the run meta.
    let routes: HashSet<String> = result.visits.iter().map(|v| route_key(&v.url)).collect();
    let meta = RunMeta {
        base_url: cli.base_url.clone(),
        started_at,
        duration_s: started.elapsed().as_secs_f64(),
        budget_s: cli.budget,
        pages_visited: result.visits.len(),
        routes_covered: routes.len(),
        controls_exercised: result.controls_exercised,
        requests_observed: result.requests_observed,
        findings_total: result.findings.len(),
        errors: result.findings.count(Severity::Error),
        warnings: result.findings.count(Severity::Warning),
    };

    report::write(&bundle, &meta, &result.findings, &result.visits)?;

    println!(
        "forage: {} pages, {} controls exercised, {} findings ({} errors, {} warnings)",
        meta.pages_visited,
        meta.controls_exercised,
        meta.findings_total,
        meta.errors,
        meta.warnings
    );
    println!("report: {}", bundle.dir.join("report.html").display());

    Ok(meta.errors > 0)
}
