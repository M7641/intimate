//! The priority frontier — what to do next, ordered to favour breadth of path
//! coverage. Also home to the salvaged calque URL helpers.
//!
//! Two task kinds share one max-heap. A brand-new *route* (a path never seen)
//! outranks exercising a control, which outranks revisiting a known path with a
//! different query — so when the budget runs short, the time was spent on the
//! widest set of distinct paths first.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use url::{Origin, Url};

use crate::models::ControlSpec;

const PRIORITY_NEW_ROUTE: u8 = 3;
const PRIORITY_EXERCISE: u8 = 2;
const PRIORITY_PERMUTATION: u8 = 1;

#[derive(Debug, Clone)]
pub enum Task {
    Visit { url: String, depth: usize },
    Exercise { url: String, control: ControlSpec },
}

struct Prioritized {
    priority: u8,
    seq: u64,
    task: Task,
}

impl PartialEq for Prioritized {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.seq == other.seq
    }
}
impl Eq for Prioritized {}
impl Ord for Prioritized {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first; within a band, lower seq first (FIFO ≈ BFS).
        self.priority
            .cmp(&other.priority)
            .then(other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for Prioritized {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct Frontier {
    heap: BinaryHeap<Prioritized>,
    seq: u64,
    origin: Origin,
    /// URLs already queued (push-time dedupe).
    seen_urls: HashSet<String>,
    /// Routes already queued — drives the new-route priority bump.
    seen_routes: HashSet<String>,
    /// Control signatures already queued (url|selector).
    exercised: HashSet<String>,
    /// Visits made per route — caps query permutations.
    per_route: HashMap<String, usize>,
    max_per_route: usize,
    max_depth: usize,
}

impl Frontier {
    pub fn new(origin: Origin, max_per_route: usize, max_depth: usize) -> Self {
        Self {
            heap: BinaryHeap::new(),
            seq: 0,
            origin,
            seen_urls: HashSet::new(),
            seen_routes: HashSet::new(),
            exercised: HashSet::new(),
            per_route: HashMap::new(),
            max_per_route,
            max_depth,
        }
    }

    fn push(&mut self, priority: u8, task: Task) {
        self.heap.push(Prioritized {
            priority,
            seq: self.seq,
            task,
        });
        self.seq += 1;
    }

    /// Queue a link to visit, if it's same-origin, http(s), within depth, and
    /// not already queued. New routes get priority.
    pub fn push_visit(&mut self, url: &str, depth: usize) {
        if depth > self.max_depth {
            return;
        }
        if !same_origin(url, &self.origin) || !http_scheme(url) {
            return;
        }
        let norm = normalize(url);
        if !self.seen_urls.insert(norm) {
            return;
        }
        let route = route_key(url);
        let priority = if self.seen_routes.insert(route) {
            PRIORITY_NEW_ROUTE
        } else {
            PRIORITY_PERMUTATION
        };
        self.push(
            priority,
            Task::Visit {
                url: url.to_string(),
                depth,
            },
        );
    }

    /// Queue exercising a control on a page, if not already queued.
    pub fn push_exercise(&mut self, url: &str, control: ControlSpec) {
        let sig = format!("{}|{}", normalize(url), control.selector);
        if !self.exercised.insert(sig) {
            return;
        }
        self.push(
            PRIORITY_EXERCISE,
            Task::Exercise {
                url: url.to_string(),
                control,
            },
        );
    }

    /// Pop the next task, enforcing the per-route cap on visits (a capped visit
    /// is dropped and the next task returned instead).
    pub fn pop(&mut self) -> Option<Task> {
        while let Some(item) = self.heap.pop() {
            if let Task::Visit { ref url, .. } = item.task {
                let route = route_key(url);
                let count = self.per_route.entry(route).or_insert(0);
                if *count >= self.max_per_route {
                    continue;
                }
                *count += 1;
            }
            return Some(item.task);
        }
        None
    }
}

// ── URL helpers (salvaged from calque/src/crawl.rs) ──────────────────────────

pub fn same_origin(url: &str, origin: &Origin) -> bool {
    Url::parse(url)
        .map(|u| &u.origin() == origin)
        .unwrap_or(false)
}

fn http_scheme(url: &str) -> bool {
    Url::parse(url)
        .map(|u| matches!(u.scheme(), "http" | "https"))
        .unwrap_or(false)
}

/// Drop the fragment so `/a` and `/a#x` count as one URL.
pub fn normalize(url: &str) -> String {
    match Url::parse(url) {
        Ok(mut u) => {
            u.set_fragment(None);
            u.to_string()
        }
        Err(_) => url.to_string(),
    }
}

/// The route a URL belongs to — its path, ignoring query and fragment.
pub fn route_key(url: &str) -> String {
    Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.to_string())
}

pub fn slugify(url: &str) -> String {
    let path = Url::parse(url)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| url.to_string());
    let slug: String = path
        .trim_matches('/')
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if slug.is_empty() {
        "root".to_string()
    } else {
        slug
    }
}
