use std::io::Write;

use crate::ui;

/// Default chain of impeccable skills for an existing codebase.
/// Source: https://impeccable.style/ — recommended flow is
/// `audit` → `polish` → `critique`.
const DEFAULT_CHAIN: &[&str] = &["audit", "polish", "critique"];

const VALID_SKILLS: &[&str] = &[
    "audit", "polish", "critique", "typeset", "colorize", "animate", "layout", "bolder", "quieter",
    "delight", "detect", "document", "shape", "craft", "teach", "live", "redo",
];

pub async fn run(
    paths: &[String],
    chain: Option<String>,
    auto: bool,
    save: bool,
) -> anyhow::Result<()> {
    let chain = resolve_chain(chain.as_deref())?;

    if paths.is_empty() {
        anyhow::bail!("no files selected");
    }

    ui::status(&format!(
        "running impeccable chain: {} (over {} file{})",
        chain.join(" → "),
        paths.len(),
        if paths.len() == 1 { "" } else { "s" },
    ));

    let timestamp = now_timestamp().replace(' ', "_").replace(':', "");
    let mut step_outputs: Vec<(String, String)> = Vec::new();

    for (idx, skill) in chain.iter().enumerate() {
        ui::status(&format!(
            "step {}/{}: /impeccable {skill}",
            idx + 1,
            chain.len(),
        ));

        let output = invoke_skill(skill, paths).await?;
        ui::response(&output);
        step_outputs.push((skill.clone(), output));

        let is_last = idx == chain.len() - 1;
        if !is_last && !auto && !confirm_continue(&chain[idx + 1])? {
            ui::status("chain stopped by user");
            break;
        }
    }

    if save && !step_outputs.is_empty() {
        let report_path = write_report(&timestamp, paths, &step_outputs)?;
        ui::status(&format!("report saved → {report_path}"));
    }

    Ok(())
}

fn resolve_chain(chain: Option<&str>) -> anyhow::Result<Vec<String>> {
    let raw: Vec<String> = match chain {
        Some(s) => s.split(',').map(|s| s.trim().to_string()).collect(),
        None => DEFAULT_CHAIN.iter().map(|s| s.to_string()).collect(),
    };

    for skill in &raw {
        if !VALID_SKILLS.contains(&skill.as_str()) {
            anyhow::bail!(
                "unknown impeccable skill: '{skill}'. valid: {}",
                VALID_SKILLS.join(", ")
            );
        }
    }

    if raw.is_empty() {
        anyhow::bail!("chain is empty");
    }

    Ok(raw)
}

/// Invoke `/impeccable <skill>` via the Claude Code CLI.
///
/// Skills like `polish` need to edit files, so we pass `--permission-mode
/// acceptEdits` — without it the subprocess would silently stall waiting for
/// approvals it can't surface to the user.
async fn invoke_skill(skill: &str, paths: &[String]) -> anyhow::Result<String> {
    let file_list = paths.join(", ");
    let prompt = format!("/impeccable {skill}\n\nFocus this run on these files: {file_list}",);

    let output = tokio::process::Command::new("claude")
        .args([
            "-p",
            &prompt,
            "--output-format",
            "text",
            "--permission-mode",
            "acceptEdits",
        ])
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("failed to run `claude` CLI ({e}) — is it installed?"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("/impeccable {skill} failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn confirm_continue(next_skill: &str) -> anyhow::Result<bool> {
    ui::prompt(&format!("continue to /impeccable {next_skill}? [Y/n]"));
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(matches!(
        input.trim().to_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

fn write_report(
    timestamp: &str,
    paths: &[String],
    steps: &[(String, String)],
) -> anyhow::Result<String> {
    let report_path = format!("nyx-impeccable-{timestamp}.md");

    let mut content = format!("# Impeccable Report — {timestamp}\n\n");
    content.push_str("**Files:**\n");
    for p in paths {
        content.push_str(&format!("- `{p}`\n"));
    }
    content.push('\n');

    for (skill, output) in steps {
        content.push_str(&format!("## /impeccable {skill}\n\n"));
        content.push_str(output);
        content.push_str("\n\n");
    }

    std::fs::write(&report_path, &content)?;
    Ok(report_path)
}

fn now_timestamp() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
