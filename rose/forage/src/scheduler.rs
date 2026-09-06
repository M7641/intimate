//! The core loop: drive the frontier within the global budget, turning visits
//! and interactions into findings until time runs out.

use std::collections::HashSet;

use anyhow::Result;
use chromiumoxide::Page;
use tracing::info;
use url::Url;

use crate::budget::Budget;
use crate::bundle::Bundle;
use crate::capture::console::ConsoleCapture;
use crate::capture::network::NetworkCapture;
use crate::capture::visual;
use crate::explore::{controls, page};
use crate::findings::{Category, Finding, Findings, Severity};
use crate::frontier::{slugify, Frontier, Task};
use crate::models::VisitRecord;
use crate::safety::{self, Decision};
use crate::{validate, RunConfig};

pub struct RunResult {
    pub findings: Findings,
    pub visits: Vec<VisitRecord>,
    pub requests_observed: usize,
    pub controls_exercised: usize,
}

pub struct Scheduler<'a> {
    page: &'a Page,
    net: &'a NetworkCapture,
    console: &'a ConsoleCapture,
    budget: &'a Budget,
    bundle: &'a Bundle,
    cfg: &'a RunConfig,
    frontier: Frontier,

    net_seen: HashSet<String>,
    console_cursor: usize,
    findings: Findings,
    visits: Vec<VisitRecord>,
    controls_exercised: usize,
    /// The page URL the browser is currently on (so we re-navigate only when an
    /// Exercise task targets a different page).
    current_url: Option<String>,
    /// Counter for evidence screenshot filenames.
    shot_seq: usize,
}

impl<'a> Scheduler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        page: &'a Page,
        net: &'a NetworkCapture,
        console: &'a ConsoleCapture,
        budget: &'a Budget,
        bundle: &'a Bundle,
        cfg: &'a RunConfig,
    ) -> Result<Self> {
        let origin = Url::parse(&cfg.base_url)?.origin();
        Ok(Self {
            page,
            net,
            console,
            budget,
            bundle,
            cfg,
            frontier: Frontier::new(origin, cfg.max_per_route, cfg.max_depth),
            net_seen: HashSet::new(),
            console_cursor: 0,
            findings: Findings::default(),
            visits: Vec::new(),
            controls_exercised: 0,
            current_url: None,
            shot_seq: 0,
        })
    }

    pub async fn run(mut self) -> RunResult {
        self.frontier.push_visit(&self.cfg.base_url, 0);

        while !self.budget.expired() {
            if self.visits.len() >= self.cfg.max_pages {
                info!(
                    max_pages = self.cfg.max_pages,
                    "page budget reached — stopping"
                );
                break;
            }
            let Some(task) = self.frontier.pop() else {
                info!("frontier drained — nothing left to explore");
                break;
            };
            self.budget.throttle().await;
            match task {
                Task::Visit { url, depth } => self.handle_visit(&url, depth).await,
                Task::Exercise { url, control } => {
                    match safety::decide(&control, self.cfg.submit_forms) {
                        Decision::Skip(reason) => self.note_skip(&url, &control, reason),
                        Decision::Exercise if self.cfg.dry_run => {
                            self.note_skip(&url, &control, "dry-run: would exercise")
                        }
                        Decision::Exercise => self.handle_exercise(&url, control).await,
                    }
                }
            }
        }

        RunResult {
            findings: self.findings,
            visits: self.visits,
            requests_observed: self.net_seen.len(),
            controls_exercised: self.controls_exercised,
        }
    }

    async fn handle_visit(&mut self, url: &str, depth: usize) {
        info!(%url, depth, remaining = ?self.budget.remaining(), "visit");
        let visit = page::visit(
            self.page,
            url,
            depth,
            self.net,
            self.console,
            self.budget,
            &mut self.net_seen,
            &mut self.console_cursor,
        )
        .await;
        self.current_url = Some(url.to_string());

        let before = self.findings.len();
        self.findings.extend(validate::page(&visit));
        self.capture_evidence_if_needed(before).await;

        // Enqueue discovered links and (unless disabled) controls.
        for link in &visit.links {
            self.frontier.push_visit(link, depth + 1);
        }
        let mut exercisable = 0;
        if self.cfg.exercise {
            for control in &visit.controls {
                self.frontier.push_exercise(url, control.clone());
                exercisable += 1;
            }
        }

        self.visits.push(VisitRecord {
            url: url.to_string(),
            depth,
            status: visit.status,
            title: visit.title.clone(),
            controls_found: visit.controls.len(),
            controls_exercised: exercisable, // queued; refined below is overkill for a prototype
            timed_out: visit.timed_out,
        });
    }

    async fn handle_exercise(&mut self, url: &str, control: crate::models::ControlSpec) {
        // Re-navigate if the browser drifted from the control's page.
        if self.current_url.as_deref() != Some(url) {
            if (tokio::time::timeout(self.budget.action_timeout(), self.page.goto(url)).await)
                .is_err()
            {
                return;
            }
            let _ = self.page.wait_for_navigation().await;
            self.net
                .wait_for_idle(controls::IDLE_QUIET, self.budget.action_timeout())
                .await;
            self.current_url = Some(url.to_string());
            // Drain the re-navigation traffic so it isn't attributed to the action.
            let _ = self.net.delta(&mut self.net_seen).await;
            let _ = self.console.delta(&mut self.console_cursor).await;
        }

        let token = crate::capture::dom::page_token(self.page).await;
        let outcome = controls::exercise(
            self.page,
            &control,
            self.net,
            self.console,
            self.budget,
            &mut self.net_seen,
            &mut self.console_cursor,
            token.as_deref(),
        )
        .await;
        self.controls_exercised += 1;

        // A navigation may have left us on a new URL.
        self.current_url = outcome.navigated_to.clone().or(self.current_url.take());

        let before = self.findings.len();
        self.findings.extend(validate::interaction(&outcome, url));
        self.capture_evidence_if_needed(before).await;

        // Pagination and the like reveal new links.
        for link in &outcome.revealed_links {
            self.frontier.push_visit(link, 0);
        }
    }

    fn note_skip(&mut self, url: &str, control: &crate::models::ControlSpec, reason: &str) {
        self.findings.add(
            Finding::new(Category::Skipped, Severity::Info, url, reason.to_string())
                .with_control(control.clone()),
        );
    }

    /// If the findings added since `before` include a visual category, take one
    /// screenshot and attach it.
    async fn capture_evidence_if_needed(&mut self, before: usize) {
        if !self.findings.has_visual_error(before) {
            return;
        }
        self.shot_seq += 1;
        let name = format!(
            "{:03}-{}",
            self.shot_seq,
            slugify(self.current_url.as_deref().unwrap_or("page"))
        );
        let (path, rel) = self.bundle.evidence_path(&name);
        if visual::full_page(self.page, &path).await.is_ok() {
            // Attach to whichever visual finding lacks a shot.
            self.findings
                .attach_screenshot(Category::ControlError, &rel);
            self.findings.attach_screenshot(Category::PageError, &rel);
        }
    }
}
