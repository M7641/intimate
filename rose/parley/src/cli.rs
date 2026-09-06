//! parley's command-line front door.
//!
//! The backend binary is the app: `serve` runs the API (the production
//! entrypoint, and the default with no subcommand), and the other commands
//! orchestrate the SolidJS frontend for local dev — so a contributor drives the
//! whole app through one binary instead of juggling `cargo` and `npm` by hand.
//!
//! `serve` is handled in `main.rs` (it owns the tokio runtime); everything else
//! is a synchronous process-spawning helper dispatched from here.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "parley",
    about = "parley — conversational language tutor: serve the API, or run the frontend for local dev.",
    version
)]
pub struct Cli {
    /// With no subcommand the binary serves the API (the production entrypoint).
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand, Clone)]
pub enum CliCommand {
    /// Run the API server (default).
    Serve,
    /// Install frontend dependencies (npm install).
    Install,
    /// Build the frontend for production (npm ci + npm run build).
    Build,
    /// Run the frontend dev server only (npm run dev).
    Dev,
    /// Run frontend (Vite) + backend together for local development.
    Start {
        /// Install frontend deps first (chains install + start).
        #[arg(long)]
        install: bool,
        /// Disable backend hot-reload (default: reload via bacon if installed).
        #[arg(long)]
        no_reload: bool,
    },
}

/// Run a non-`serve` command; returns the process exit code to propagate.
pub fn dispatch(command: CliCommand) -> i32 {
    match command {
        // `serve` boots tokio, so `main` handles it directly.
        CliCommand::Serve => unreachable!("serve is handled in main"),
        CliCommand::Install => install(),
        CliCommand::Build => build(),
        CliCommand::Dev => dev(),
        CliCommand::Start { install, no_reload } => start(install, no_reload),
    }
}

/// The crate root (where `Cargo.toml` and `frontend/` live), fixed at compile time.
fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn frontend_dir() -> PathBuf {
    project_root().join("frontend")
}

// --- minimal terminal output (no external UI crate) ---

fn step(cmd: &str, cwd: &Path) {
    eprintln!("\x1b[36m▸\x1b[0m {cmd}  \x1b[2m({})\x1b[0m", cwd.display());
}

fn note(msg: &str) {
    eprintln!("\x1b[2m· {msg}\x1b[0m");
}

fn error(msg: &str) {
    eprintln!("\x1b[31m✗ {msg}\x1b[0m");
}

/// Run a command in `cwd`, echoing it; returns its exit code (127 if unspawnable).
fn run(cmd: &[&str], cwd: &Path) -> i32 {
    step(&cmd.join(" "), cwd);
    match Command::new(cmd[0]).args(&cmd[1..]).current_dir(cwd).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            error(&format!("failed to run {}: {err}", cmd[0]));
            127
        }
    }
}

fn install() -> i32 {
    run(&["npm", "install"], &frontend_dir())
}

fn build() -> i32 {
    let code = run(&["npm", "ci"], &frontend_dir());
    if code != 0 {
        return code;
    }
    run(&["npm", "run", "build"], &frontend_dir())
}

fn dev() -> i32 {
    run(&["npm", "run", "dev"], &frontend_dir())
}

/// Run Vite + the backend together for local dev.
///
/// Vite is spawned in its own process group so the whole tree (npm → node) can
/// be signalled at teardown; the backend runs in the foreground, and Vite is
/// stopped once the backend exits.
fn start(install_first: bool, no_reload: bool) -> i32 {
    if install_first {
        let code = install();
        if code != 0 {
            return code;
        }
    }

    let backend_cmd = backend_command(!no_reload);

    step("npm run dev", &frontend_dir());
    let mut vite = match spawn_process_group(&["npm", "run", "dev"], &frontend_dir()) {
        Ok(child) => child,
        Err(err) => {
            error(&format!("failed to start Vite: {err}"));
            return 1;
        }
    };

    step(&backend_cmd.join(" "), project_root());
    let backend_status = Command::new(backend_cmd[0])
        .args(&backend_cmd[1..])
        .current_dir(project_root())
        .status();

    // Teardown: stop Vite's process group.
    note("stopping frontend dev server");
    terminate_group(vite.id());
    let _ = vite.wait();

    match backend_status {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            error(&format!("backend failed: {err}"));
            1
        }
    }
}

/// Pick the command that runs the backend, hot-reloading via bacon by default.
///
/// `bacon --headless -j serve` re-runs `serve` on each change; without bacon we
/// fall back to a one-shot `cargo run -- serve` (no reload). `--no-reload` takes
/// that same one-shot path on purpose.
fn backend_command(want_watch: bool) -> Vec<&'static str> {
    const ONE_SHOT: [&str; 4] = ["cargo", "run", "--", "serve"];

    if !want_watch {
        return ONE_SHOT.to_vec();
    }
    if command_succeeds("bacon", &["--version"]) {
        return vec!["bacon", "--headless", "-j", "serve"];
    }
    note("bacon not found — running once without hot-reload (cargo install bacon to enable it)");
    ONE_SHOT.to_vec()
}

/// Whether a probe command runs successfully — used to detect optional dev tools.
fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Spawn a command as the leader of its own process group (Unix).
fn spawn_process_group(cmd: &[&str], cwd: &Path) -> std::io::Result<std::process::Child> {
    let mut command = Command::new(cmd[0]);
    command.args(&cmd[1..]).current_dir(cwd);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn()
}

/// Send SIGTERM to the whole process group `pid` (Unix).
fn terminate_group(pid: u32) {
    #[cfg(unix)]
    // SAFETY: killpg on a valid pgid; failure (group already gone) is benign.
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    let _ = pid;
}
