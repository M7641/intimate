use std::process::Command;

pub fn staged_diff() -> anyhow::Result<String> {
    let output = Command::new("git").args(["diff", "--cached"]).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn stage_all() -> anyhow::Result<()> {
    Command::new("git").args(["add", "-A"]).status()?;
    Ok(())
}

pub fn commit(message: &str) -> anyhow::Result<()> {
    let status = Command::new("git")
        .args(["commit", "-m", message])
        .status()?;
    if !status.success() {
        anyhow::bail!("git commit failed");
    }
    Ok(())
}
