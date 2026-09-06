use clap::CommandFactory;

use crate::ui;
use std::path::PathBuf;

const MARKER: &str = "# nyx shell completions";

struct ShellConfig {
    name: &'static str,
    shell: clap_complete::Shell,
    rc_path: PathBuf,
    completions_path: PathBuf,
}

/// Detect the user's shell, generate a completions file, and source it from the rc file.
pub fn run() -> anyhow::Result<()> {
    let config = detect_shell()?;

    ui::status(&format!("detected shell: {}", config.name));

    // Generate the completions file
    let completions_dir = nyx_data_dir()?;
    std::fs::create_dir_all(&completions_dir)?;

    let mut buf = Vec::new();
    clap_complete::generate(config.shell, &mut crate::Cli::command(), "nyx", &mut buf);
    std::fs::write(&config.completions_path, &buf)?;

    ui::status(&format!(
        "wrote completions to {}",
        config.completions_path.display()
    ));

    let source_line = format!("source \"{}\"", config.completions_path.display());

    if config.rc_path.exists() {
        let content = std::fs::read_to_string(&config.rc_path)?;

        // Check for the correct source line already present
        if content.contains(&source_line) {
            ui::status("completions already set up — nothing to do");
            return Ok(());
        }

        // Check for stale nyx entries that the user needs to clean up
        if content.contains("nyx completions") || content.contains(MARKER) {
            ui::status("found outdated nyx completions in your rc file");
            ui::status(&format!(
                "please remove the old nyx lines from {} and re-run nyx init",
                config.rc_path.display()
            ));
            return Ok(());
        }
    }

    // First time — append
    let block = format!("\n{MARKER}\n{source_line}\n");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.rc_path)?;
    std::io::Write::write_all(&mut file, block.as_bytes())?;

    ui::status(&format!(
        "added source line to {}",
        config.rc_path.display()
    ));
    ui::status("restart your shell or run:");
    ui::status(&format!("  source {}", config.rc_path.display()));

    Ok(())
}

/// Refresh the completions file if `nyx init` has been run before.
/// Called on every nyx invocation to keep completions in sync.
pub fn refresh_if_needed() {
    let Ok(config) = detect_shell() else { return };

    if !config.completions_path.exists() {
        return;
    }

    let mut buf = Vec::new();
    clap_complete::generate(config.shell, &mut crate::Cli::command(), "nyx", &mut buf);

    // Only write if content actually changed (avoids unnecessary IO + mtime churn)
    let current = std::fs::read(&config.completions_path).unwrap_or_default();
    if current != buf {
        std::fs::write(&config.completions_path, &buf).ok();
    }
}

fn detect_shell() -> anyhow::Result<ShellConfig> {
    let shell_env = std::env::var("SHELL").unwrap_or_default();
    let home = home_dir()?;
    let data = nyx_data_dir()?;

    let shell_name = shell_env.rsplit('/').next().unwrap_or("");

    match shell_name {
        "zsh" => Ok(ShellConfig {
            name: "zsh",
            shell: clap_complete::Shell::Zsh,
            rc_path: home.join(".zshrc"),
            completions_path: data.join("nyx.zsh"),
        }),
        "bash" => {
            let rc = if cfg!(target_os = "macos") && !home.join(".bashrc").exists() {
                home.join(".bash_profile")
            } else {
                home.join(".bashrc")
            };
            Ok(ShellConfig {
                name: "bash",
                shell: clap_complete::Shell::Bash,
                rc_path: rc,
                completions_path: data.join("nyx.bash"),
            })
        }
        "fish" => Ok(ShellConfig {
            name: "fish",
            shell: clap_complete::Shell::Fish,
            rc_path: home.join(".config/fish/config.fish"),
            completions_path: data.join("nyx.fish"),
        }),
        other => {
            let name = if other.is_empty() {
                "$SHELL is not set"
            } else {
                other
            };
            anyhow::bail!("unsupported shell: {name}");
        }
    }
}

fn home_dir() -> anyhow::Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| anyhow::anyhow!("$HOME is not set"))
}

fn nyx_data_dir() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?.join(".nyx"))
}
