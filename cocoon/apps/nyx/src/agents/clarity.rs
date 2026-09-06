use std::sync::Arc;

use crate::llm::LlmClient;
use crate::ui;

const PREAMBLE: &str = "\
You are a code clarity auditor. Your job is to scan source code and identify \
sections that would confuse a new reader and need more explanation.

Flag things like:
- Magic numbers or string literals with no obvious meaning in context
- Complex boolean expressions or bitwise operations without comments
- Non-obvious control flow (e.g. early returns that are easy to miss, implicit fallthrough)
- Variable/function names that are misleading given what they actually do
- Implicit type conversions or casts that could surprise a reader
- Regex patterns without explanation
- Silent error swallowing (empty catch blocks, `.ok()` on critical results)
- Non-obvious side effects

DO NOT flag:
- Code that is self-explanatory from context (e.g. `let count = items.len()`)
- Standard idioms of the language (e.g. `unwrap()` in tests, `if __name__ == \"__main__\"`)
- Well-named constants even if the value seems arbitrary (e.g. `const MAX_RETRIES: u32 = 3`)
- Simple arithmetic or string formatting
- Imports, module declarations, or standard boilerplate

Respond with ONLY a JSON array (no markdown fences, no explanation). Each element:
{
  \"line\": <line number or start line>,
  \"code\": \"<the problematic snippet, max ~80 chars>\",
  \"reason\": \"<why this is unclear, 1 sentence>\",
  \"confidence\": <1-10, where 10 = extremely confusing>
}

If the code is perfectly clear, return an empty array: []
Sort by confidence descending (most confusing first).";

const MODEL: &str = "opus";

/// Result of scanning a single file.
pub struct FileReport {
    pub path: String,
    pub findings: Vec<Finding>,
}

#[derive(serde::Deserialize)]
pub struct Finding {
    pub line: u32,
    pub code: String,
    pub reason: String,
    pub confidence: u8,
}

/// Scan multiple files, printing results and appending to a markdown report.
///
/// When `parallel` is true, all files are scanned concurrently (good for cloud LLMs).
/// When false, files are scanned sequentially (better for local models).
const MAX_FILES: usize = 5;

pub async fn run(
    llm: Arc<dyn LlmClient>,
    paths: &[String],
    parallel: bool,
    save: bool,
) -> anyhow::Result<()> {
    if paths.len() > MAX_FILES {
        anyhow::bail!(
            "maximum {MAX_FILES} files per run (got {}). split into multiple runs to conserve LLM usage",
            paths.len()
        );
    }

    ui::status(&format!(
        "scanning {} file{} ({})",
        paths.len(),
        if paths.len() == 1 { "" } else { "s" },
        if parallel { "parallel" } else { "sequential" },
    ));

    let reports = if parallel {
        scan_parallel(llm, paths).await
    } else {
        scan_sequential(&*llm, paths).await
    };

    // Write markdown report if requested
    let total_findings: usize = reports.iter().map(|r| r.findings.len()).sum();
    let saved_path = if save && total_findings > 0 {
        Some(write_markdown_report(&reports)?)
    } else {
        None
    };

    // Render all reports + summary in a single viewport (no cursor drift)
    ui::clarity_results(&reports, saved_path.as_deref());

    Ok(())
}

/// Scan files concurrently using tokio::spawn.
async fn scan_parallel(llm: Arc<dyn LlmClient>, paths: &[String]) -> Vec<FileReport> {
    let mut handles = Vec::new();

    for path in paths {
        let llm = Arc::clone(&llm);
        let path = path.clone();
        handles.push(tokio::spawn(async move { scan_file(&*llm, &path).await }));
    }

    let mut reports = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(report)) => reports.push(report),
            Ok(Err(e)) => ui::status(&format!("scan error: {e}")),
            Err(e) => ui::status(&format!("task panicked: {e}")),
        }
    }

    reports
}

/// Scan files one at a time.
async fn scan_sequential(llm: &dyn LlmClient, paths: &[String]) -> Vec<FileReport> {
    let mut reports = Vec::new();

    for path in paths {
        match scan_file(llm, path).await {
            Ok(report) => reports.push(report),
            Err(e) => ui::status(&format!("scan error for {path}: {e}")),
        }
    }

    reports
}

/// Scan a single file and return its findings.
async fn scan_file(llm: &dyn LlmClient, path: &str) -> anyhow::Result<FileReport> {
    let content =
        std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("cannot read {path}: {e}"))?;

    if content.trim().is_empty() {
        return Ok(FileReport {
            path: path.to_string(),
            findings: Vec::new(),
        });
    }

    let numbered: String = content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:>4} │ {line}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");

    let filename = std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    ui::status(&format!("scanning {filename}..."));

    let prompt = format!(
        "Scan this file ({filename}) and identify code that needs more explanation:\n\n{numbered}"
    );

    let raw = llm
        .prompt_with_model(PREAMBLE, &prompt, Some(MODEL))
        .await?;

    let findings = parse_findings(&raw)?;

    Ok(FileReport {
        path: path.to_string(),
        findings,
    })
}

fn parse_findings(raw: &str) -> anyhow::Result<Vec<Finding>> {
    let json_str = raw.trim();
    let json_str = json_str
        .strip_prefix("```json")
        .or_else(|| json_str.strip_prefix("```"))
        .unwrap_or(json_str);
    let json_str = json_str.strip_suffix("```").unwrap_or(json_str).trim();

    serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("failed to parse LLM response: {e}\nRaw:\n{raw}"))
}

/// Write scan results to a timestamped markdown file.
fn write_markdown_report(reports: &[FileReport]) -> anyhow::Result<String> {
    let timestamp = now_timestamp();
    let file_timestamp = timestamp.replace(' ', "_").replace(':', "");
    let report_path = format!("nyx-clarity-{file_timestamp}.md");

    let mut content = format!(
        "# Clarity Report — {timestamp}\n\n\
         Generated by `nyx clarity`. Findings ranked by confidence — higher means more confusing.\n\n"
    );

    for report in reports {
        if report.findings.is_empty() {
            continue;
        }

        content.push_str(&format!("## `{}`\n\n", report.path));
        content.push_str("| Line | Confidence | Code | Reason |\n");
        content.push_str("|------|-----------|------|--------|\n");

        for f in &report.findings {
            let code = f.code.replace('|', "\\|");
            let reason = f.reason.replace('|', "\\|");
            content.push_str(&format!(
                "| L{} | {}/10 | `{}` | {} |\n",
                f.line, f.confidence, code, reason
            ));
        }

        content.push('\n');
    }

    std::fs::write(&report_path, &content)?;

    Ok(report_path)
}

fn now_timestamp() -> String {
    // Use the `date` command for a simple timestamp without adding chrono as a dep
    std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
