//! `warden` command definitions and execution — the cocoon control layer.
//!
//! `warden` deploys the *other* cocoon apps into the Nimbus tenant it is run
//! against. The tenant is whichever `API_KEY` is in the environment: `warden`
//! reads that one deploy token and nothing else, so it deploys "into the tenant
//! it is running in" without ever holding a data credential.
//!
//! Every app's deploy shape comes from `deploy-catalog`, the single source of
//! truth shared with each app's own `deploy` subcommand.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use deploy_catalog::{AppSpec, SecretMode};

use crate::ui;

#[derive(Parser)]
#[command(
    name = "warden",
    about = "cocoon control layer: deploy the apps into the tenant you run against.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the deployable apps in the catalogue (and, with --live, which are
    /// already deployed in the tenant).
    List {
        /// Cross-reference the tenant: mark apps already deployed (needs API_KEY).
        #[arg(long)]
        live: bool,
    },
    /// Show what a deploy would do, without contacting Nimbus (no API_KEY needed).
    Plan {
        #[command(flatten)]
        target: TargetArgs,
        /// Resolve runtime secrets from this environment instead of by reference.
        #[arg(long)]
        from_env: bool,
    },
    /// Build and deploy one app (or --all) into the current tenant.
    Deploy {
        #[command(flatten)]
        target: TargetArgs,
        /// Resolve runtime secrets from this environment and forward them to the
        /// build. Off by default: warden deploys by reference and holds no data
        /// credential (the relaxed control layer).
        #[arg(long)]
        from_env: bool,
    },
}

/// Which app(s) a command acts on: a single name, or `--all`.
#[derive(clap::Args)]
struct TargetArgs {
    /// The app to act on (e.g. `snowhouse`). Omit and pass --all for every app.
    #[arg(required_unless_present = "all")]
    app: Option<String>,
    /// Act on every app in the catalogue.
    #[arg(long, conflicts_with = "app")]
    all: bool,
}

impl TargetArgs {
    /// Resolve to the concrete list of specs, or an error message to show.
    fn resolve(&self) -> Result<Vec<&'static AppSpec>, String> {
        if self.all {
            return Ok(deploy_catalog::all().iter().collect());
        }
        let name = self.app.as_deref().expect("clap guarantees app or --all");
        match deploy_catalog::find(name) {
            Some(spec) => Ok(vec![spec]),
            None => Err(format!(
                "unknown app '{name}'. Known apps: {}",
                known_apps().join(", ")
            )),
        }
    }
}

/// Run the CLI, returning the process exit code.
pub fn run() -> i32 {
    match Cli::parse().command {
        Command::List { live } => list(live),
        Command::Plan { target, from_env } => plan(&target, secret_mode(from_env)),
        Command::Deploy { target, from_env } => deploy(&target, secret_mode(from_env)),
    }
}

fn secret_mode(from_env: bool) -> SecretMode {
    if from_env {
        SecretMode::FromEnv
    } else {
        SecretMode::ByReference
    }
}

/// List catalogue apps; with `live`, mark those already deployed in the tenant.
fn list(live: bool) -> i32 {
    ui::header("catalogue");

    // A tenant cross-reference needs the deploy token and one API call.
    let deployed = if live {
        match load_deployed_service_names() {
            Ok(names) => Some(names),
            Err(err) => {
                ui::error(&format!("could not read the tenant: {err}"));
                return 1;
            }
        }
    } else {
        None
    };

    for spec in deploy_catalog::all() {
        let marker = match &deployed {
            Some(names) if names.contains(&service_name(spec)) => "● live ",
            Some(_) => "○ absent",
            None => "",
        };
        let creds = if spec.runtime_secrets.is_empty() {
            "no data credentials".to_string()
        } else {
            format!("needs {}", spec.runtime_secrets.join(", "))
        };
        ui::note(&format!(
            "{marker:<8}{:<12} {:<7} {:<6} — {creds}",
            spec.name,
            spec.service_type.as_nimbus_str(),
            spec.version,
        ));
    }
    0
}

/// Dry-run: print the resolved deploy for each target without contacting Nimbus.
fn plan(target: &TargetArgs, mode: SecretMode) -> i32 {
    let specs = match target.resolve() {
        Ok(specs) => specs,
        Err(err) => {
            ui::error(&err);
            return 1;
        }
    };

    ui::header("plan");
    for spec in specs {
        let ds = spec.to_deploy_service(deploy_catalog::DEFAULT_TARGET, workspace_root(), mode);
        println!();
        ui::note(&format!("{} → service {}", spec.name, service_name(spec)));
        ui::note(&format!("  dockerfile   {}", ds.dockerfile_path));
        let build_args: Vec<String> = ds
            .build_args
            .iter()
            .map(|a| format!("{}={}", a.name, a.value))
            .collect();
        ui::note(&format!("  build args   {}", join_or_dash(&build_args)));
        ui::note(&format!("  build secrets {}", join_or_dash(&ds.build_secrets)));
        report_runtime_secrets(spec, mode);
    }
    0
}

/// Deploy each target into the current tenant via `ouroboros`.
fn deploy(target: &TargetArgs, mode: SecretMode) -> i32 {
    let specs = match target.resolve() {
        Ok(specs) => specs,
        Err(err) => {
            ui::error(&err);
            return 1;
        }
    };

    // One authenticated client for the whole run — this IS the tenant selector.
    let client = match ouroboros::Client::from_env() {
        Ok(client) => client,
        Err(err) => {
            ui::error(&format!("{err} (set API_KEY to the target tenant's token)"));
            return 1;
        }
    };

    ui::header("deploy");
    if mode == SecretMode::FromEnv {
        ui::warn(
            "--from-env: warden will read data credentials from this environment \
             and bake them into the images (the transitional path).",
        );
    }

    let mut failures = 0;
    for spec in specs {
        println!();
        ui::note(&format!("deploying {} → {}", spec.name, service_name(spec)));
        report_runtime_secrets(spec, mode);

        let opts = spec.to_deploy_service(deploy_catalog::DEFAULT_TARGET, workspace_root(), mode);
        match ouroboros::deploy_service(&client, &opts) {
            Ok(id) => ui::success(&format!("{} deployed (service id {id})", spec.name)),
            Err(err) => {
                ui::error(&format!("{} failed: {err}", spec.name));
                failures += 1;
            }
        }
    }

    if failures > 0 {
        ui::error(&format!("{failures} deploy(s) failed"));
        return 1;
    }
    ui::success("all deploys completed");
    0
}

/// Warn about runtime secrets this deploy does not satisfy — the tenant must
/// provide them to the running service by other means.
fn report_runtime_secrets(spec: &AppSpec, mode: SecretMode) {
    let unsatisfied = spec.unsatisfied_runtime_secrets(mode);
    if !unsatisfied.is_empty() {
        ui::warn(&format!(
            "  by reference: the tenant must provide [{}] to the running service",
            unsatisfied.join(", ")
        ));
    }
}

/// The Nimbus service name `ouroboros` derives, e.g. `snowhouse-v1-cocoon`.
fn service_name(spec: &AppSpec) -> String {
    format!(
        "{}-{}-{}",
        spec.name.replace('_', "-").to_lowercase(),
        spec.version,
        deploy_catalog::DEFAULT_TARGET
    )
}

/// Names of services currently deployed in the tenant.
fn load_deployed_service_names() -> ouroboros::Result<Vec<String>> {
    let client = ouroboros::Client::from_env()?;
    let services = ouroboros::Services::new(&client).list()?;
    Ok(services
        .iter()
        .filter_map(|s| s.get("name").and_then(|v| v.as_str()).map(str::to_string))
        .collect())
}

fn known_apps() -> Vec<&'static str> {
    deploy_catalog::all().iter().map(|s| s.name).collect()
}

fn join_or_dash(items: &[String]) -> String {
    if items.is_empty() {
        "—".to_string()
    } else {
        items.join(", ")
    }
}

/// The Cargo workspace root (`cocoon/`), the Docker build context for a deploy.
///
/// `warden` lives at `<workspace>/apps/warden`, so its `CARGO_MANIFEST_DIR`
/// climbs two levels to the workspace — the same path each app's `deploy` uses.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CARGO_MANIFEST_DIR should be <workspace>/apps/warden")
        .to_path_buf()
}
