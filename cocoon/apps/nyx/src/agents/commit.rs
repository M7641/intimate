use crate::llm::LlmClient;
use crate::tools::git;
use crate::ui;

const PREAMBLE: &str = "\
You generate git commit messages following the Conventional Commits specification.
Format: <type>(<scope>): <description>

Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.
The scope is optional but should specify what part of the codebase is affected.
The description should be concise but specific, including relevant keywords and
technical terms (function names, file types, configuration settings) useful for
searching commit history later. Use lowercase, do not end with a period.
If the changes are significant, include a body after a blank line.
Return only the commit message, no explanation or formatting.

In the final output, do not include any explanation or formatting.
";

const MODEL: &str = "haiku";

pub async fn run(llm: &dyn LlmClient) -> anyhow::Result<()> {
    git::stage_all()?;

    let diff = git::staged_diff()?;
    if diff.is_empty() {
        ui::status("no changes to commit");
        return Ok(());
    }

    let prompt = format!("Generate a commit message for this diff:\n\n{diff}");
    let message = llm
        .prompt_with_model(PREAMBLE, &prompt, Some(MODEL))
        .await?;
    let message = message.trim();

    ui::commit_preview(message);

    ui::prompt("[Y/n/e(dit)]");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    match input.trim().to_lowercase().as_str() {
        "" | "y" | "yes" => {
            git::commit(message)?;
            ui::status("committed!");
        }
        "e" | "edit" => {
            let edited = edit_message(message)?;
            if edited.is_empty() {
                ui::status("commit aborted (empty message)");
                return Ok(());
            }
            git::commit(&edited)?;
            ui::status("committed!");
        }
        _ => {
            ui::status("commit aborted");
        }
    }

    Ok(())
}

fn edit_message(initial: &str) -> anyhow::Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let tmp = std::env::temp_dir().join("nyx-commit-msg.txt");
    std::fs::write(&tmp, initial)?;

    let status = std::process::Command::new(&editor).arg(&tmp).status()?;

    if !status.success() {
        anyhow::bail!("editor exited with error");
    }

    let result = std::fs::read_to_string(&tmp)?;
    std::fs::remove_file(&tmp).ok();
    Ok(result.trim().to_string())
}
