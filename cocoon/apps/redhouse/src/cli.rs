//! CLI unifiée de redhouse : serveur, dev local et déploiement.
//!
//! Port Rust de l'ancien `src/warehouse/cli.py` (typer). Le binaire Rust est
//! désormais l'unique point d'entrée :
//! - `serve` lance le serveur API — voir `main.rs` ;
//! - `install`/`build`/`dev`/`start` orchestrent le dev local (bun + cargo) ;
//! - `deploy` empaquette et déploie l'image via la lib `ouroboros`.
//!
//! La sortie terminale stylée est mutualisée dans [`service_kit::cli_ui`].

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};

use clap::{Parser, Subcommand};
use service_kit::cli_ui;

const APP: &str = env!("CARGO_PKG_NAME");

#[derive(Parser)]
#[command(
    name = "redhouse",
    about = "redhouse — analytics app: serve, local dev, deploy.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Clone)]
pub enum CliCommand {
    /// Run the API server.
    Serve,
    /// Install frontend dependencies (bun install).
    Install,
    /// Build the frontend for production (bun install --frozen-lockfile + build).
    Build,
    /// Run the frontend dev server only (bun run dev).
    Dev,
    /// Start frontend (vite) + backend together for local development.
    Start {
        /// Install frontend deps first (chains install + start).
        #[arg(long)]
        install: bool,
        /// Hot-reload the backend on change (needs cargo-watch).
        #[arg(long)]
        reload: bool,
    },
    /// Build and deploy the container image to Nimbus.
    Deploy,
}

/// Exécute une commande hors `serve` et renvoie le code de sortie du processus.
pub fn dispatch(command: CliCommand) -> i32 {
    match command {
        // `serve` est traité directement dans `main` (il démarre tokio).
        CliCommand::Serve => unreachable!("serve is handled in main"),
        CliCommand::Install => install(),
        CliCommand::Build => build(),
        CliCommand::Dev => dev(),
        CliCommand::Start { install, reload } => start(install, reload),
        CliCommand::Deploy => deploy(),
    }
}

/// Racine du projet (où vivent `Cargo.toml` et `frontend/`), figée à la compilation.
fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Racine du workspace Cargo (`cocoon/`), deux niveaux au-dessus de l'app.
///
/// C'est le contexte de build Docker : le `Cargo.toml` de l'app hérite du
/// `[workspace.package]` / `[workspace.dependencies]` racine (`*.workspace =
/// true`), donc l'image doit embarquer tout le workspace pour que
/// `cargo build -p redhouse` trouve sa racine. `CARGO_MANIFEST_DIR` vaut
/// `cocoon/apps/redhouse` → `../..` = `cocoon`.
fn workspace_root() -> &'static Path {
    project_root()
        .parent()
        .and_then(Path::parent)
        .expect("CARGO_MANIFEST_DIR should be <workspace>/apps/<app>")
}

fn frontend_dir() -> PathBuf {
    project_root().join("frontend")
}

/// Les commandes de dev local exigent un checkout source (Cargo.toml + frontend/).
fn require_checkout() -> bool {
    if project_root().join("Cargo.toml").exists() {
        return true;
    }
    cli_ui::error(&format!(
        "Cargo.toml not found at {}; local-dev commands must run from a source checkout.",
        project_root().display()
    ));
    false
}

/// Lance une commande dans `cwd`, en l'affichant ; renvoie son code de sortie.
fn run(cmd: &[&str], cwd: &Path) -> i32 {
    if !require_checkout() {
        return 1;
    }
    cli_ui::step(&cmd.join(" "), cwd);
    match Command::new(cmd[0])
        .args(&cmd[1..])
        .current_dir(cwd)
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            cli_ui::error(&format!("failed to run {}: {err}", cmd[0]));
            127
        }
    }
}

fn install() -> i32 {
    run(&["bun", "install"], &frontend_dir())
}

fn build() -> i32 {
    let code = run(&["bun", "install", "--frozen-lockfile"], &frontend_dir());
    if code != 0 {
        return code;
    }
    run(&["bun", "run", "build"], &frontend_dir())
}

fn dev() -> i32 {
    run(&["bun", "run", "dev"], &frontend_dir())
}

/// Lance vite (frontend) + le backend ensemble pour le dev local.
///
/// Vite est spawné dans son propre groupe de processus pour pouvoir signaler tout
/// l'arbre (bun → node) au teardown ; le backend tourne au premier plan, et vite
/// est arrêté quand le backend se termine.
fn start(install_first: bool, reload: bool) -> i32 {
    if !require_checkout() {
        return 1;
    }

    cli_ui::header(APP, "local dev");

    if install_first {
        let code = run(&["bun", "install"], &frontend_dir());
        if code != 0 {
            return code;
        }
    }

    // Start the backend FIRST, in its own process group, and wait until it is
    // actually serving before opening the dev dashboard — so the frontend never
    // races the API and shows connection errors on first load. The server only
    // binds its port after the DB connection succeeds, so "listening" == "ready".
    // Redshift-only: the connector (postgres) is the default feature, no selection.
    let backend_cmd = backend_command(reload);
    cli_ui::step(&backend_cmd.join(" "), project_root());
    let backend_args: Vec<&str> = backend_cmd.iter().map(String::as_str).collect();
    let mut backend = match spawn_process_group(&backend_args, project_root()) {
        Ok(child) => child,
        Err(err) => {
            cli_ui::error(&format!("failed to start backend: {err}"));
            return 1;
        }
    };

    // From here on, Ctrl-C must tear the backend down: it lives in its own
    // process group, so the terminal's SIGINT never reaches it. Install the
    // handler now, so even Ctrl-C during the first (slow) compile cleans up.
    install_sigint_teardown(backend.id());

    if !wait_for_backend(&mut backend) {
        terminate_group(backend.id());
        let _ = backend.wait();
        return 1;
    }
    cli_ui::success("backend is up — starting the dev dashboard");

    cli_ui::step("bun run dev", &frontend_dir());
    let vite_status = Command::new("bun")
        .args(["run", "dev"])
        .current_dir(frontend_dir())
        .status();

    // Teardown: stop the backend's process group (bacon → cargo → server).
    cli_ui::note("stopping backend");
    terminate_group(backend.id());
    let _ = backend.wait();

    match vite_status {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            cli_ui::error(&format!("vite failed: {err}"));
            1
        }
    }
}

/// The command that runs the backend, hot-reloading via bacon by default.
///
/// With `reload` (and bacon installed) we run bacon `--headless` (no TUI, so it
/// coexists with the foreground vite) on an inline `serve` job. Otherwise a
/// one-shot `cargo run`. The Postgres/Redshift connector is the default feature.
fn backend_command(reload: bool) -> Vec<String> {
    if reload && command_succeeds("bacon", &["--version"]) {
        let job = "[jobs.serve]\n\
             command = [\"cargo\", \"run\", \"--\", \"serve\"]\n\
             need_stdout = true\n\
             background = false\n\
             on_change_strategy = \"kill_then_restart\"\n\
             kill = [\"kill\", \"-s\", \"INT\"]\n"
            .to_string();
        return vec![
            "bacon".into(),
            "--headless".into(),
            "-j".into(),
            "serve".into(),
            "--config-toml".into(),
            job,
        ];
    }
    if reload {
        cli_ui::note(
            "bacon not found — running once without hot-reload. \
             Install it with `cargo install bacon`.",
        );
    }
    vec!["cargo".into(), "run".into(), "--".into(), "serve".into()]
}

/// Poll until the backend is serving, or it exits / we time out.
///
/// The first build can take a while (bacon compiles, then the server connects to
/// the warehouse), so we allow a generous window.
fn wait_for_backend(backend: &mut std::process::Child) -> bool {
    const MAX_ATTEMPTS: u32 = 180; // ~3 min: first compile + DB connect

    cli_ui::note("waiting for the backend to become ready (http://localhost:8050)…");
    for _ in 0..MAX_ATTEMPTS {
        // Bail out early if the backend died (e.g. failed to connect to the DB).
        if let Ok(Some(status)) = backend.try_wait() {
            cli_ui::error(&format!("backend exited before it was ready ({status})"));
            return false;
        }
        if backend_is_ready() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    cli_ui::error("backend did not become ready in time");
    false
}

/// Whether the backend answers on :8050 — an HTTP 200 from `/health/live` if
/// `curl` is available, else a raw TCP connect (the port is only bound once the
/// server is fully initialised, so a connection is a reliable readiness signal).
fn backend_is_ready() -> bool {
    if command_succeeds(
        "curl",
        &[
            "-s",
            "-f",
            "-o",
            "/dev/null",
            "http://localhost:8050/health/live",
        ],
    ) {
        return true;
    }
    std::net::TcpStream::connect("127.0.0.1:8050").is_ok()
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

/// Spawn une commande comme chef de son propre groupe de processus (Unix).
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

/// Envoie SIGTERM à tout le groupe de processus `pid` (Unix).
fn terminate_group(pid: u32) {
    #[cfg(unix)]
    // SAFETY: killpg sur un pgid valide ; l'échec (groupe déjà parti) est bénin.
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    let _ = pid;
}

/// PGID of the spawned backend group, shared with the SIGINT handler. 0 = none.
static BACKEND_PGID: AtomicI32 = AtomicI32::new(0);

/// SIGINT handler: tear the backend group down, then exit. The backend runs in
/// its OWN process group, so the terminal's Ctrl-C never reaches it — and the
/// launcher would otherwise die on SIGINT before its normal teardown runs,
/// leaving the backend orphaned and still holding the API port. This makes
/// Ctrl-C stop the whole stack.
#[cfg(unix)]
extern "C" fn on_sigint(_sig: libc::c_int) {
    let pgid = BACKEND_PGID.load(Ordering::SeqCst);
    if pgid > 0 {
        // SAFETY: killpg is async-signal-safe; failure (group gone) is benign.
        unsafe {
            libc::killpg(pgid as libc::pid_t, libc::SIGTERM);
        }
    }
    // SAFETY: _exit is async-signal-safe. 130 = 128 + SIGINT (conventional code).
    unsafe {
        libc::_exit(130);
    }
}

/// Record the backend group and install the SIGINT teardown handler.
fn install_sigint_teardown(backend_pgid: u32) {
    #[cfg(unix)]
    {
        BACKEND_PGID.store(backend_pgid as i32, Ordering::SeqCst);
        // SAFETY: on_sigint is async-signal-safe; installing a handler is sound.
        unsafe {
            libc::signal(libc::SIGINT, on_sigint as *const () as libc::sighandler_t);
        }
    }
    #[cfg(not(unix))]
    let _ = backend_pgid;
}

/// Empaquette et déploie l'image vers Nimbus via `ouroboros`.
fn deploy() -> i32 {
    // Fait remonter la progression (build, polling) loggée par ouroboros.
    cli_ui::init_logging();
    cli_ui::header(APP, "deploy → cocoon");

    let client = match ouroboros::Client::from_env() {
        Ok(client) => client,
        Err(err) => {
            cli_ui::error(&format!("deploy error: {err}"));
            return 1;
        }
    };

    // Deploy shape comes from the shared `deploy-catalog` — the single source of
    // truth, read by both this subcommand and the `warden` control CLI. Running
    // in `FromEnv` mode preserves this path's historical behaviour: the Redshift
    // credentials are read from the environment and forwarded to the image build
    // (the Dockerfile promotes them to runtime env vars for the connector).
    let opts = deploy_catalog::find(APP)
        .expect("redhouse is registered in deploy-catalog")
        .to_deploy_service(
            deploy_catalog::DEFAULT_TARGET,
            workspace_root().to_path_buf(),
            deploy_catalog::SecretMode::FromEnv,
        );

    match ouroboros::deploy_service(&client, &opts) {
        Ok(_) => {
            cli_ui::success("Image deployed");
            0
        }
        Err(err) => {
            cli_ui::error(&format!("deploy error: {err}"));
            1
        }
    }
}
