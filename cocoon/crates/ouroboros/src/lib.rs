//! `ouroboros` — client + CLI (lecture seule) pour la plateforme Nimbus.
//!
//! Port Rust de `blank/ouroboros`. Le crate expose, comme le package Python :
//!
//! - une **librairie** : un [`Client`] HTTP authentifié et quatre vues de domaine
//!   ([`Images`], [`Services`], [`Workflows`], [`Tenant`]) ;
//! - un **binaire** `ouroboros` (voir [`cli`]) reproduisant les 7 commandes de
//!   lecture (`list-*`, `describe-*`, `instance-options`).
//!
//! # Exemple
//!
//! ```rust,no_run
//! use ouroboros::{Client, Services};
//!
//! # fn main() -> ouroboros::Result<()> {
//! let client = Client::from_env()?; // lit API_KEY
//! let services = Services::new(&client).list()?;
//! println!("{} services", services.len());
//! # Ok(())
//! # }
//! ```

mod artifact;
pub mod cli;
pub mod client;
pub mod error;
pub mod images;
pub mod services;
pub mod tenant;
pub mod types;
pub mod workflows;

pub use client::Client;
pub use error::{Error, Result};
pub use images::{Images, deploy_image};
pub use services::{DeployService, Services, deploy_service};
pub use tenant::Tenant;
pub use types::{Artifact, BuildArg};
pub use workflows::Workflows;
