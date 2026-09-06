//! Single source of truth for how each cocoon app is deployed to Nimbus.
//!
//! Every deployable app is described once, declaratively, by an [`AppSpec`].
//! Two consumers read from this catalogue:
//!
//! - each app's own `deploy` subcommand (so the app deploys *itself*), and
//! - the `warden` control CLI (so it can deploy the *others* into its tenant).
//!
//! Because both read the same specs, a service's deploy shape — image name,
//! Dockerfile, credentials — is declared in exactly one place and cannot drift
//! between the two paths.
//!
//! # Two layers, two credential natures
//!
//! A cocoon deployment splits in two:
//!
//! - the **control layer** (`warden`) holds only the tenant's Nimbus `API_KEY` —
//!   a *deploy* token, scoped to the tenant it runs in. It is deliberately
//!   relaxed: it carries no data credentials.
//! - the **application layer** (`snowhouse`, `redhouse`, …) is tenant- and
//!   credential-specific: each service needs warehouse credentials to read data.
//!
//! So an [`AppSpec`] separates two kinds of secret:
//!
//! - [`AppSpec::build_secrets`] — build-time secret *references* Nimbus mounts
//!   during the image build and never bakes in (e.g. `GITHUB_TOKEN`).
//! - [`AppSpec::runtime_secrets`] — secrets the *running* service needs
//!   (e.g. `REDSHIFT_USERNAME`). These are the data credentials, declared here
//!   BY REFERENCE — as names, never values.
//!
//! How a runtime secret is *satisfied* at deploy time is a [`SecretMode`]:
//! either left for the tenant to provide ([`SecretMode::ByReference`], the
//! relaxed control-layer default) or read from the deployer's environment and
//! forwarded ([`SecretMode::FromEnv`], the current image-baking compromise the
//! per-app `deploy` still uses).

use std::path::PathBuf;

use ouroboros::{Artifact, BuildArg, DeployService};

/// Logical grouping tag applied to every service (Nimbus `tags`).
///
/// Distinct from the *tenant*: the tenant is the `API_KEY` the deployer holds,
/// whereas the target is just a label grouping these services on the platform.
pub const DEFAULT_TARGET: &str = "cocoon";

/// The kind of Nimbus service an app deploys as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceType {
    /// A web application (has a frontend served on a public URL).
    Webapp,
    /// A headless HTTP API.
    Api,
}

impl ServiceType {
    /// The string `ouroboros::DeployService` expects (`"webapp"` / `"api"`).
    #[must_use]
    pub fn as_nimbus_str(self) -> &'static str {
        match self {
            ServiceType::Webapp => "webapp",
            ServiceType::Api => "api",
        }
    }
}

/// How the runtime secrets of an app are satisfied at deploy time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretMode {
    /// By reference: the deployer reads no secret values. Runtime secrets are
    /// left for the tenant to provide to the running service. This keeps the
    /// control layer relaxed — it never transports a data credential — and is
    /// `warden`'s default. See [`AppSpec::unsatisfied_runtime_secrets`] for the
    /// wiring that this mode still leaves to the platform.
    ByReference,
    /// Transitional: read each runtime secret from the deployer's environment
    /// and forward it as a build argument (the current image-baking compromise,
    /// documented in each app's Dockerfile). This is what an app's own `deploy`
    /// subcommand uses, so its behaviour is unchanged by this catalogue.
    FromEnv,
}

/// Declarative deploy spec for one cocoon app.
///
/// Every field is a compile-time constant; nothing secret ever appears here —
/// secrets are named ([`Self::runtime_secrets`] / [`Self::build_secrets`]), not
/// valued.
#[derive(Debug, Clone)]
pub struct AppSpec {
    /// Cargo package name, e.g. `"snowhouse"`. Also the base of the service name.
    pub name: &'static str,
    /// Whether this deploys as a webapp or a headless API.
    pub service_type: ServiceType,
    /// Service version suffix (`"v1"`, `"v2"`), part of the Nimbus service name.
    pub version: &'static str,
    /// Dockerfile path, relative to the artifact root (the workspace root).
    pub dockerfile_path: &'static str,
    /// Non-secret build arguments, as `(name, value)` pairs (e.g. `TARGET_ENV`).
    pub build_args: &'static [(&'static str, &'static str)],
    /// Build-time secret references Nimbus mounts during the build and never bakes
    /// into the image (e.g. `GITHUB_TOKEN`). Passed through by reference always.
    pub build_secrets: &'static [&'static str],
    /// Names of the secrets the *running* service needs (its data credentials),
    /// declared BY REFERENCE. Satisfied per [`SecretMode`] at deploy time.
    pub runtime_secrets: &'static [&'static str],
}

impl AppSpec {
    /// Build the `ouroboros` deploy spec for this app.
    ///
    /// `artifact_root` is the directory uploaded as the build context (the Cargo
    /// workspace root, since every app inherits the root manifest). Runtime
    /// secrets are satisfied per `mode`.
    #[must_use]
    pub fn to_deploy_service(
        &self,
        target: &str,
        artifact_root: PathBuf,
        mode: SecretMode,
    ) -> DeployService {
        let mut build_args: Vec<BuildArg> = self
            .build_args
            .iter()
            .map(|(name, value)| BuildArg::new(*name, *value))
            .collect();

        // FromEnv resolves each runtime secret from the deployer's environment
        // and forwards it as a build arg (the Dockerfile promotes it to runtime
        // env). ByReference forwards nothing — the value never touches the
        // deployer; see `unsatisfied_runtime_secrets`.
        if mode == SecretMode::FromEnv {
            for name in self.runtime_secrets {
                let value = std::env::var(name).unwrap_or_default();
                build_args.push(BuildArg::new(*name, value));
            }
        }

        DeployService {
            service_name: self.name.to_string(),
            service_type: self.service_type.as_nimbus_str().to_string(),
            target: target.to_string(),
            build_args,
            build_secrets: self.build_secrets.iter().map(|s| s.to_string()).collect(),
            dockerfile_path: self.dockerfile_path.to_string(),
            version: self.version.to_string(),
            // Build context = workspace root, honouring `<root>/.dockerignore`.
            artifact: Some(Artifact {
                path: artifact_root,
                ignore_files: vec![".dockerignore".to_string()],
            }),
            scale_to_zero: false,
            ..Default::default()
        }
    }

    /// Runtime secrets this deploy does NOT satisfy — the tenant must provide
    /// them to the running service by other means (platform env / secret store).
    ///
    /// Empty under [`SecretMode::FromEnv`] (all forwarded from the environment),
    /// and the full list under [`SecretMode::ByReference`]. `warden` prints these
    /// so an operator knows exactly what the tenant still owes each service.
    #[must_use]
    pub fn unsatisfied_runtime_secrets(&self, mode: SecretMode) -> &'static [&'static str] {
        match mode {
            SecretMode::ByReference => self.runtime_secrets,
            SecretMode::FromEnv => &[],
        }
    }
}

/// Every deployable cocoon app. Adding an app here makes both its own `deploy`
/// subcommand and `warden` aware of it.
const CATALOG: &[AppSpec] = &[
    AppSpec {
        name: "data_view",
        service_type: ServiceType::Webapp,
        version: "v2",
        dockerfile_path: "apps/data_view/Dockerfile",
        build_args: &[("TARGET_ENV", "cocoon")],
        build_secrets: &["GITHUB_TOKEN"],
        runtime_secrets: &[],
    },
    AppSpec {
        name: "redhouse",
        service_type: ServiceType::Webapp,
        version: "v1",
        dockerfile_path: "apps/redhouse/Dockerfile",
        build_args: &[("TARGET_ENV", "cocoon")],
        build_secrets: &[],
        runtime_secrets: &["REDSHIFT_USERNAME", "REDSHIFT_PASSWORD"],
    },
    AppSpec {
        name: "snowhouse",
        service_type: ServiceType::Webapp,
        version: "v1",
        dockerfile_path: "apps/snowhouse/Dockerfile",
        build_args: &[("TARGET_ENV", "cocoon")],
        build_secrets: &[],
        runtime_secrets: &["API_KEY"],
    },
];

/// All deployable apps, in catalogue order.
#[must_use]
pub fn all() -> &'static [AppSpec] {
    CATALOG
}

/// Look up an app's spec by its package name (e.g. `"snowhouse"`).
#[must_use]
pub fn find(name: &str) -> Option<&'static AppSpec> {
    CATALOG.iter().find(|spec| spec.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_hits_and_misses() {
        assert_eq!(find("snowhouse").map(|s| s.name), Some("snowhouse"));
        assert!(find("does-not-exist").is_none());
    }

    #[test]
    fn all_names_are_unique() {
        let mut names: Vec<&str> = all().iter().map(|s| s.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate app name in the catalogue");
    }

    #[test]
    fn service_name_and_dockerfile_map_through() {
        let spec = find("snowhouse").unwrap();
        let ds = spec.to_deploy_service("cocoon", PathBuf::from("/ws"), SecretMode::ByReference);
        assert_eq!(ds.service_name, "snowhouse");
        assert_eq!(ds.service_type, "webapp");
        assert_eq!(ds.version, "v1");
        assert_eq!(ds.dockerfile_path, "apps/snowhouse/Dockerfile");
        assert_eq!(ds.target, "cocoon");
    }

    #[test]
    fn by_reference_never_forwards_a_runtime_secret() {
        // The whole point of the relaxed control layer: values stay out of the
        // deploy. Only the non-secret build args survive.
        let spec = find("redhouse").unwrap();
        let ds = spec.to_deploy_service("cocoon", PathBuf::from("/ws"), SecretMode::ByReference);
        let arg_names: Vec<&str> = ds.build_args.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(arg_names, vec!["TARGET_ENV"]);
        assert_eq!(
            spec.unsatisfied_runtime_secrets(SecretMode::ByReference),
            &["REDSHIFT_USERNAME", "REDSHIFT_PASSWORD"]
        );
    }

    #[test]
    fn from_env_forwards_runtime_secrets_as_build_args() {
        let spec = find("redhouse").unwrap();
        let ds = spec.to_deploy_service("cocoon", PathBuf::from("/ws"), SecretMode::FromEnv);
        let arg_names: Vec<&str> = ds.build_args.iter().map(|a| a.name.as_str()).collect();
        assert!(arg_names.contains(&"REDSHIFT_USERNAME"));
        assert!(arg_names.contains(&"REDSHIFT_PASSWORD"));
        assert!(spec.unsatisfied_runtime_secrets(SecretMode::FromEnv).is_empty());
    }

    #[test]
    fn build_secrets_pass_through_by_name() {
        let spec = find("data_view").unwrap();
        let ds = spec.to_deploy_service("cocoon", PathBuf::from("/ws"), SecretMode::ByReference);
        assert_eq!(ds.build_secrets, vec!["GITHUB_TOKEN".to_string()]);
    }
}
