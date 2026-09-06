use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::llm::LlmClient;
use crate::ui;

const PREAMBLE: &str = "\
You are a software scope analyst writing for a director or executive audience. \
Your job is to read a codebase and produce a list of business deliverables — \
the user-facing capabilities the software provides, framed in business terms.

Each deliverable has three parts:
1. \"deliverable\" — a short (<= 8 words), plain-English label for what has been delivered.
2. \"description\" — 1 to 3 sentences in more depth: what the capability does, what \
   business problem it solves, and what business levers it moves (efficiency, revenue, \
   risk, visibility, time-to-decision, etc.). No code references, no jargon.
3. \"assumptions\" — what the customer must provide for the deliverable to work. \
   This may be data (e.g. \"a daily sales feed in CSV\"), access (e.g. \"read-only \
   credentials to their warehouse\"), or process (e.g. \"users review the output \
   weekly\"). Be concrete.

Rules:
- Order deliverables by business impact, highest first.
- Combine related code into one deliverable rather than listing every function.
- Skip pure plumbing (logging, build scripts, test harness) unless they ARE the product.
- DO NOT include tab characters or newline characters inside any field — they break TSV.
- Respond with ONLY a JSON array (no markdown fences, no prose).

Each element must look like:
{\"deliverable\": \"...\", \"description\": \"...\", \"assumptions\": \"...\"}

If the codebase is empty or unreadable, return [].";

const MODEL: &str = "sonnet";

#[derive(serde::Deserialize)]
struct Deliverable {
    deliverable: String,
    description: String,
    assumptions: String,
}

const CONTEXT_BUDGET: usize = 80 * 1024;

const SOURCE_EXTS: &[&str] = &[
    "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "rb", "kt", "swift", "toml", "yaml", "yml",
    "json", "md",
];

const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    "documentation",
    ".next",
    ".turbo",
];

/// Entry point for the scope agent.
pub async fn run(llm: Arc<dyn LlmClient>, root: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    if !root.exists() {
        anyhow::bail!("path does not exist: {}", root.display());
    }

    ui::status(&format!("scanning {} for deliverables...", root.display()));

    let context = gather_context(&root);
    if context.trim().is_empty() {
        anyhow::bail!("no readable source files found in {}", root.display());
    }

    ui::status(&format!(
        "sending {:.1} KB of context to LLM...",
        context.len() as f64 / 1024.0,
    ));

    let user_prompt = format!(
        "Analyse this codebase rooted at `{}` and identify the business deliverables it provides:\n\n{context}",
        root.display(),
    );

    let raw = llm
        .prompt_with_model(PREAMBLE, &user_prompt, Some(MODEL))
        .await?;
    let deliverables = parse_deliverables(&raw)?;

    if deliverables.is_empty() {
        ui::status("no deliverables identified");
        return Ok(());
    }

    write_tsv(&output, &deliverables)?;
    let shown = std::fs::canonicalize(&output).unwrap_or_else(|_| output.clone());
    ui::status(&format!(
        "wrote {} deliverable{} to {}",
        deliverables.len(),
        if deliverables.len() == 1 { "" } else { "s" },
        shown.display(),
    ));

    Ok(())
}

fn parse_deliverables(raw: &str) -> anyhow::Result<Vec<Deliverable>> {
    let start = raw
        .find('[')
        .ok_or_else(|| anyhow::anyhow!("LLM response did not contain a JSON array:\n{raw}"))?;
    let end = raw
        .rfind(']')
        .ok_or_else(|| anyhow::anyhow!("LLM response did not contain a JSON array:\n{raw}"))?;
    let json_slice = &raw[start..=end];

    serde_json::from_str(json_slice)
        .map_err(|e| anyhow::anyhow!("failed to parse LLM response: {e}\nRaw:\n{raw}"))
}

fn write_tsv(path: &Path, deliverables: &[Deliverable]) -> anyhow::Result<()> {
    let mut out = String::from("Deliverable\tDescription\tAssumptions\n");
    for d in deliverables {
        out.push_str(&format!(
            "{}\t{}\t{}\n",
            sanitize(&d.deliverable),
            sanitize(&d.description),
            sanitize(&d.assumptions),
        ));
    }
    std::fs::write(path, out)?;
    Ok(())
}

/// Collapse any tab/newline/whitespace runs into single spaces — guarantees
/// each TSV row stays on one line and uses tabs only as column separators.
fn sanitize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn gather_context(root: &Path) -> String {
    let mut files = Vec::new();
    walk(root, &mut files);

    files.sort_by_key(|p| {
        let name = p
            .file_name()
            .map(|f| f.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with("readme") {
            0
        } else if matches!(ext.as_str(), "toml" | "json" | "yaml" | "yml") {
            1
        } else if ext == "md" {
            2
        } else {
            3
        }
    });

    let mut parts = Vec::new();
    let mut budget = CONTEXT_BUDGET;

    for file in files {
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        let display = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .display()
            .to_string();

        if content.len() > budget {
            parts.push(format!(
                "--- {display} (truncated) ---\n{}",
                &content[..budget]
            ));
            break;
        }

        budget -= content.len();
        parts.push(format!("--- {display} ---\n{content}"));

        if budget == 0 {
            break;
        }
    }

    parts.join("\n\n")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            if SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            walk(&path, out);
        } else {
            let lower_name = name_str.to_lowercase();
            let ext_ok = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| SOURCE_EXTS.contains(&e))
                .unwrap_or(false);
            if ext_ok || lower_name.starts_with("readme") {
                out.push(path);
            }
        }
    }
}
