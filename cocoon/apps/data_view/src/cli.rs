//! CLI unifiée de data_view : serveur, dev local et déploiement.
//!
//! Port Rust de l'ancien `src/data_view/cli.py` (typer). Le binaire Rust est
//! désormais l'unique point d'entrée :
//! - `serve` lance le serveur API — voir `main.rs` ;
//! - `install`/`build`/`dev`/`start` orchestrent le dev local (bun + cargo) ;
//! - `deploy` empaquette et déploie l'image via la lib `ouroboros`.
//!
//! La sortie terminale stylée est mutualisée dans [`service_kit::cli_ui`].

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};
use service_kit::cli_ui;

const APP: &str = env!("CARGO_PKG_NAME");

#[derive(Parser)]
#[command(
    name = "data_view",
    about = "data_view — snapshot data viewer: serve, local dev, deploy.",
    version
)]
pub struct Cli {
    /// Optional: with no subcommand the binary serves the API (the production
    /// entrypoint), matching `ENTRYPOINT ["./data_view"]`.
    #[command(subcommand)]
    pub command: Option<CliCommand>,
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
        /// Disable backend hot-reload (default: reload via bacon).
        #[arg(long)]
        no_reload: bool,
        /// Serve against a seeded throwaway Postgres container instead of the
        /// real warehouse — offline dev / fixtures. By default `start` uses the
        /// warehouse from the ambient env (DATA_WAREHOUSE_TYPE + REDSHIFT_*/
        /// SNOWFLAKE_*), same as `serve`.
        #[arg(long)]
        test: bool,
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
        CliCommand::Start {
            install,
            no_reload,
            test,
        } => start(install, no_reload, test),
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
/// `cargo build -p data_view` trouve sa racine. `CARGO_MANIFEST_DIR` vaut
/// `cocoon/apps/data_view` → `../..` = `cocoon`.
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

/// Name + host port for the throwaway dev database container.
const DEV_PG_CONTAINER: &str = "data_view-dev-pg";
const DEV_PG_PORT: &str = "55432";

/// First reachable container engine (`docker`, else `podman`), or `None`.
///
/// We talk to whichever CLI answers `info`, so Podman works without the
/// `DOCKER_HOST` dance — its `run`/`exec`/`rm` verbs match Docker's.
fn container_engine() -> Option<&'static str> {
    ["docker", "podman"].into_iter().find(|engine| {
        Command::new(engine)
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Pipe SQL into `psql` inside the dev container over stdin.
fn psql_exec(engine: &str, sql: &str) -> bool {
    let child = Command::new(engine)
        .args([
            "exec",
            "-i",
            DEV_PG_CONTAINER,
            "psql",
            "-U",
            "postgres",
            "-d",
            "postgres",
            "-q",
            "-v",
            "ON_ERROR_STOP=1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(_) => return false,
    };
    let Some(mut stdin) = child.stdin.take() else {
        return false;
    };
    if stdin.write_all(sql.as_bytes()).is_err() {
        return false;
    }
    drop(stdin); // close the pipe so psql sees EOF and runs
    child.wait().map(|s| s.success()).unwrap_or(false)
}

/// Boot a throwaway Postgres container and seed it with the same SQL the
/// integration tests use (schema + fixtures + Redshift-compat shims). The SQL is
/// baked in via `include_str!`, so there's no runtime path dependence.
fn start_dev_database(engine: &str) -> bool {
    // Clear any container left over from a prior run (self-heals a leak).
    let _ = Command::new(engine)
        .args(["rm", "-f", DEV_PG_CONTAINER])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    cli_ui::step(
        &format!("{engine} run postgres ({DEV_PG_CONTAINER})"),
        project_root(),
    );
    let started = Command::new(engine)
        .args([
            "run",
            "-d",
            "--name",
            DEV_PG_CONTAINER,
            "-p",
            &format!("{DEV_PG_PORT}:5432"),
            "-e",
            "POSTGRES_PASSWORD=postgres",
            "docker.io/library/postgres:18-alpine",
        ])
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !started {
        cli_ui::error("failed to start the Postgres container");
        return false;
    }

    // Wait for the server to accept connections.
    let ready = (0..30).any(|_| {
        let ok = Command::new(engine)
            .args(["exec", DEV_PG_CONTAINER, "pg_isready", "-U", "postgres"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        ok
    });
    if !ready {
        cli_ui::error("Postgres container did not become ready in time");
        return false;
    }

    cli_ui::step("seeding dev data", project_root());
    let seeds = [
        (
            "01_schema.sql",
            include_str!("../tests/seeds/01_schema.sql"),
        ),
        (
            "02_fixtures.sql",
            include_str!("../tests/seeds/02_fixtures.sql"),
        ),
        (
            "03_redshift_compat.sql",
            include_str!("../tests/seeds/03_redshift_compat.sql"),
        ),
    ];
    for (name, sql) in seeds {
        if !psql_exec(engine, sql) {
            cli_ui::error(&format!("seed {name} failed"));
            return false;
        }
    }
    true
}

/// Remove the dev database container.
fn stop_dev_database(engine: &str) {
    let _ = Command::new(engine)
        .args(["rm", "-f", DEV_PG_CONTAINER])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Lance vite (frontend) + le backend ensemble pour le dev local.
///
/// Par défaut, le backend tape le **vrai warehouse** depuis l'env
/// (DATA_WAREHOUSE_TYPE + REDSHIFT_*/SNOWFLAKE_*), comme `serve`. Avec `--test`,
/// on monte à la place une base Postgres jetable et seedée (chemin Redshift
/// filaire) — dev hors-ligne sur fixtures, sans toucher au warehouse.
///
/// Vite est spawné dans son propre groupe de processus pour pouvoir signaler tout
/// l'arbre (bun → node) au teardown ; le backend tourne au premier plan, et vite
/// (et la base de test, le cas échéant) sont arrêtés quand le backend se termine.
fn start(install_first: bool, no_reload: bool, test: bool) -> i32 {
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

    // Default: real warehouse from the ambient env (`engine` stays `None`). With
    // `--test`, bring up a throwaway seeded Postgres container and serve it via
    // the Redshift-compatible wire backend instead.
    let engine = if test {
        let engine = match container_engine() {
            Some(engine) => engine,
            None => {
                cli_ui::error(
                    "no container engine reachable for --test — start Docker \
                     Desktop, or run `podman machine start`",
                );
                return 1;
            }
        };
        if !start_dev_database(engine) {
            stop_dev_database(engine);
            return 1;
        }
        Some(engine)
    } else {
        cli_ui::note("using the warehouse from the ambient env (DATA_WAREHOUSE_TYPE)");
        None
    };

    // `start` is the dev entrypoint, so it hot-reloads the backend by default
    // (`serve` stays the one-shot prod path). `--no-reload` opts out.
    let backend_cmd = backend_command(!no_reload);

    // Start the backend FIRST, in its own process group, and wait until it is
    // actually serving before opening the dev dashboard — so the frontend never
    // races the API and shows connection errors on first load.
    cli_ui::step(&backend_cmd.join(" "), project_root());
    let mut cmd = Command::new(backend_cmd[0]);
    cmd.args(&backend_cmd[1..]).current_dir(project_root());
    // Point at the dev container only under `--test`; the default leaves the
    // ambient env (real warehouse credentials) untouched.
    if engine.is_some() {
        cmd.env("DATA_WAREHOUSE_TYPE", "amazon_redshift")
            .env("REDSHIFT_HOST", "127.0.0.1")
            .env("REDSHIFT_PORT", DEV_PG_PORT)
            .env("REDSHIFT_DATABASE", "postgres")
            .env("REDSHIFT_USERNAME", "postgres")
            .env("REDSHIFT_PASSWORD", "postgres")
            .env("REDSHIFT_SSL_MODE", "disable");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut backend = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            cli_ui::error(&format!("failed to start backend: {err}"));
            if let Some(engine) = engine {
                stop_dev_database(engine);
            }
            return 1;
        }
    };

    if !wait_for_backend(&mut backend) {
        terminate_group(backend.id());
        let _ = backend.wait();
        if let Some(engine) = engine {
            stop_dev_database(engine);
        }
        return 1;
    }
    cli_ui::success("backend is up — starting the dev dashboard");

    cli_ui::step("bun run dev", &frontend_dir());
    let vite_status = Command::new("bun")
        .args(["run", "dev"])
        .current_dir(frontend_dir())
        .status();

    // Teardown: stop the backend process group, then drop the dev database (if any).
    cli_ui::note("stopping backend");
    terminate_group(backend.id());
    let _ = backend.wait();
    if let Some(engine) = engine {
        cli_ui::note("removing dev database");
        stop_dev_database(engine);
    }

    match vite_status {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            cli_ui::error(&format!("vite failed: {err}"));
            1
        }
    }
}

/// Poll until the backend is serving, or it exits / we time out. The first build
/// can be slow (bacon compiles, then the server connects to the warehouse), so we
/// allow a generous window.
fn wait_for_backend(backend: &mut std::process::Child) -> bool {
    const MAX_ATTEMPTS: u32 = 180; // ~3 min: first compile + DB connect

    cli_ui::note("waiting for the backend to become ready (http://localhost:8050)…");
    for _ in 0..MAX_ATTEMPTS {
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

/// Pick the command that runs the backend, hot-reloading via bacon by default.
///
/// `bacon --headless -j serve` runs the `serve` job from `bacon.toml`, which
/// SIGTERMs the server on each change so the reload drains connections and
/// closes the DB pool gracefully. If bacon isn't installed we run a one-shot
/// `cargo run -- serve` (no reload) and point at the install command;
/// `--no-reload` takes that same one-shot path on purpose.
fn backend_command(want_watch: bool) -> Vec<&'static str> {
    const ONE_SHOT: [&str; 4] = ["cargo", "run", "--", "serve"];

    if !want_watch {
        return ONE_SHOT.to_vec();
    }
    if command_succeeds("bacon", &["--version"]) {
        return vec!["bacon", "--headless", "-j", "serve"];
    }
    cli_ui::note(
        "bacon not found — running once without hot-reload. \
         Install it with `cargo install bacon`, then re-run `data_view start`.",
    );
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
    // truth, read by both this subcommand and the `warden` control CLI. `FromEnv`
    // preserves this path's behaviour (data_view has no runtime credentials; its
    // GITHUB_TOKEN is a build-time secret carried by reference in the catalogue).
    let opts = deploy_catalog::find(APP)
        .expect("data_view is registered in deploy-catalog")
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
