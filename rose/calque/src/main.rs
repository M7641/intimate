//! calque — reflect a live system into a spec you can rebuild from.
//!
//! `calque ui capture <url>` is the working vertical slice: launch headless
//! Chrome, observe the network (XHR + WebSocket frames), screenshot the page,
//! and write a reflection bundle. The API path is scaffolded (see `api`).

mod api;
mod browser;
mod capture;
mod crawl;
mod detect;
mod models;
mod recipe;
mod reflect;
mod skills;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::Local;
use clap::{Parser, Subcommand};
use tracing::info;

use crate::capture::network::NetworkCapture;
use crate::models::{Meta, Reflection, Viewport};

#[derive(Parser)]
#[command(
    name = "calque",
    about = "Reflect a live system into a spec you can rebuild from"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// UI path — reflect a live web app (→ React).
    Ui {
        #[command(subcommand)]
        action: UiAction,
    },
    /// API path — reflect a live API (→ Axum).
    Api {
        #[command(subcommand)]
        action: ApiAction,
    },
    /// Manage the bundled consuming skills (reflect-to-react, reflect-to-axum).
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },
}

#[derive(Subcommand)]
enum SkillsAction {
    /// Install the bundled skills into the user's skills directory.
    Install {
        /// Target skills directory (defaults to ~/.claude/skills).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Overwrite skills that already exist.
        #[arg(long)]
        force: bool,
    },
    /// List the skills bundled in this binary.
    List,
}

#[derive(Subcommand)]
enum UiAction {
    /// Capture a live web app into a reflection bundle.
    Capture {
        /// URL of the running app to reflect.
        url: String,
        /// Interaction recipe to replay (not yet wired).
        #[arg(long)]
        recipe: Option<PathBuf>,
        /// Output root for the bundle.
        #[arg(long, default_value = "out/ui")]
        out: PathBuf,
        /// Max seconds to wait for the network to go idle before capturing.
        #[arg(long, default_value_t = 10)]
        settle: u64,
        /// Follow same-origin links from the landing page and capture each route.
        #[arg(long)]
        crawl: bool,
        /// Max routes to capture when crawling.
        #[arg(long, default_value_t = 20)]
        max_pages: usize,
        /// Max link depth to follow when crawling.
        #[arg(long, default_value_t = 2)]
        max_depth: usize,
        /// Max captures per route (same path, differing query/params).
        #[arg(long, default_value_t = 5)]
        max_per_route: usize,
    },
}

#[derive(Subcommand)]
enum ApiAction {
    /// Capture a live API into a reflection bundle.
    Capture {
        /// Base URL of the running API to reflect.
        base_url: String,
        /// OpenAPI spec to use (URL or local file). Discovered if omitted.
        #[arg(long)]
        openapi: Option<String>,
        /// Sample plan to run. Auto-derived from the spec if omitted.
        #[arg(long)]
        plan: Option<PathBuf>,
        /// Output root for the bundle.
        #[arg(long, default_value = "out/api")]
        out: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "calque=info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Ui {
            action:
                UiAction::Capture {
                    url,
                    recipe,
                    out,
                    settle,
                    crawl,
                    max_pages,
                    max_depth,
                    max_per_route,
                },
        } => {
            let crawl = crawl::Options {
                enabled: crawl,
                max_pages,
                max_depth,
                max_per_route,
            };
            run_ui_capture(&url, &out, settle, recipe, crawl).await
        }
        Command::Api {
            action:
                ApiAction::Capture {
                    base_url,
                    openapi,
                    plan,
                    out,
                },
        } => api::run_capture(&base_url, api::Options { openapi, plan, out }).await,
        Command::Skills { action } => run_skills(action),
    }
}

fn run_skills(action: SkillsAction) -> Result<()> {
    match action {
        SkillsAction::Install { dir, force } => {
            let dir = match dir {
                Some(dir) => dir,
                None => skills::default_dir()?,
            };
            let written = skills::install(&dir, force)?;
            println!("{written} skill(s) installed to {}", dir.display());
            Ok(())
        }
        SkillsAction::List => {
            skills::list();
            Ok(())
        }
    }
}

/// The UI capture flow: branch network listeners on a blank page, navigate, let
/// the app settle, snapshot DOM + a11y + screenshot, optionally replay a recipe
/// to capture reactivity, then write the bundle.
/// How long the network must stay quiet before we treat the page as loaded.
pub const IDLE_QUIET: Duration = Duration::from_millis(500);

async fn run_ui_capture(
    url: &str,
    out: &PathBuf,
    settle: u64,
    recipe_path: Option<PathBuf>,
    crawl: crawl::Options,
) -> Result<()> {
    info!(url, "launching browser");
    let session = browser::launch().await?;

    // Open blank first so listeners are attached before the target's requests.
    let page = session.browser.new_page("about:blank").await?;
    let net = NetworkCapture::start(&page).await?;

    info!(url, "navigating");
    page.goto(url).await?;
    page.wait_for_navigation().await?;
    if !net
        .wait_for_idle(IDLE_QUIET, Duration::from_secs(settle))
        .await
    {
        info!(settle, "network still busy at timeout — capturing anyway");
    }

    let html = capture::dom::capture_html(&page).await?;
    let (components, inputs) = capture::dom::capture_a11y(&page).await?;
    info!(
        components = components.len(),
        inputs = inputs.len(),
        "a11y inventory"
    );

    let bundle = reflect::new_bundle(out)?;
    capture::visual::full_page(&page, &bundle.screenshot_path("initial")).await?;
    bundle.write_html(&html)?;

    // Replay an interaction recipe, if given, to capture the reactive contract.
    let interactions = match recipe_path {
        Some(path) => {
            let recipe = recipe::load(&path)?;
            recipe::run(&page, &recipe, &net, &bundle, settle).await?
        }
        None => Vec::new(),
    };

    // Crawl the app's links, if asked, capturing each route as its own page.
    let pages = if crawl.enabled {
        info!(
            max_pages = crawl.max_pages,
            max_depth = crawl.max_depth,
            max_per_route = crawl.max_per_route,
            "crawling routes"
        );
        crawl::run(&page, url, &net, &bundle, settle, &crawl).await?
    } else {
        Vec::new()
    };

    let (network, websocket) = net.finish().await;
    let framework = detect::detect_framework(&network);
    info!(
        requests = network.len(),
        ws_frames = websocket.len(),
        interactions = interactions.len(),
        pages = pages.len(),
        ?framework,
        "capture complete"
    );

    let reflection = Reflection {
        meta: Meta {
            url: url.to_string(),
            captured_at: Local::now().to_rfc3339(),
            framework,
            viewport: Viewport::default(),
        },
        components,
        inputs,
        network,
        websocket,
        interactions,
        pages,
    };
    bundle.write_reflection(&reflection)?;

    session.close().await?;
    println!("wrote reflection bundle to {}", bundle.dir.display());
    Ok(())
}
