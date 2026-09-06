//! Définition `clap` du binaire `ouroboros` et exécution des commandes.
//!
//! Couche mince au-dessus de la librairie : chaque commande instancie une vue de
//! domaine, appelle une méthode de lecture, puis imprime ou sauvegarde le JSON.

use std::fs;

use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::client::{Client, json_id};
use crate::error::Result;
use crate::images::Images;
use crate::services::Services;
use crate::tenant::Tenant;
use crate::workflows::Workflows;

#[derive(Parser)]
#[command(
    name = "ouroboros",
    about = "Nimbus platform CLI (read-only)",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List deployed services
    ListServices {
        /// Show detailed info (one extra API call per service)
        #[arg(short, long)]
        detailed: bool,
        /// Save the result to services.json
        #[arg(short, long)]
        save: bool,
    },
    /// Describe a specific service by ID
    DescribeService {
        /// The ID of the service to describe
        service_id: String,
        /// Save the result to service_<id>.json
        #[arg(short, long)]
        save: bool,
    },
    /// List deployed workflows
    ListWorkflows {
        #[arg(short, long)]
        detailed: bool,
        #[arg(short, long)]
        save: bool,
    },
    /// Describe a specific workflow by ID
    DescribeWorkflow {
        /// The ID of the workflow to describe
        workflow_id: i64,
        #[arg(short, long)]
        save: bool,
    },
    /// List deployed images
    ListImages {
        #[arg(short, long)]
        detailed: bool,
        #[arg(short, long)]
        save: bool,
    },
    /// Describe a specific image by ID
    DescribeImage {
        /// The ID of the image to describe
        image_id: i64,
        #[arg(short, long)]
        save: bool,
    },
    /// Show available instance types and sizes for an entity type
    InstanceOptions {
        /// Entity type (e.g. workflow, webapp, api-deployment)
        entity_type: String,
    },
}

/// Parse les arguments, construit le client et exécute la commande demandée.
///
/// # Errors
/// Propage toute erreur de configuration (clé manquante), réseau, API ou I/O.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::from_env()?;

    match cli.command {
        Commands::ListServices { detailed, save } => {
            let services = Services::new(&client);
            let items = collect(services.list()?, detailed, |id| services.describe(id))?;
            output_list(&items, save, "services.json")?;
        }
        Commands::DescribeService { service_id, save } => {
            let details = Services::new(&client).describe(&service_id)?;
            output_single(&details, save, &format!("service_{service_id}.json"))?;
        }
        Commands::ListWorkflows { detailed, save } => {
            let workflows = Workflows::new(&client);
            let items = collect(workflows.list()?, detailed, |id| workflows.describe(id))?;
            output_list(&items, save, "workflows.json")?;
        }
        Commands::DescribeWorkflow { workflow_id, save } => {
            let details = Workflows::new(&client).describe(&workflow_id.to_string())?;
            output_single(&details, save, &format!("workflow_{workflow_id}.json"))?;
        }
        Commands::ListImages { detailed, save } => {
            let images = Images::new(&client);
            let items = collect(images.list()?, detailed, |id| images.describe(id))?;
            output_list(&items, save, "images.json")?;
        }
        Commands::DescribeImage { image_id, save } => {
            let details = Images::new(&client).describe(&image_id.to_string())?;
            output_single(&details, save, &format!("image_{image_id}.json"))?;
        }
        Commands::InstanceOptions { entity_type } => {
            Tenant::new(&client).print_instance_options(&entity_type)?;
        }
    }

    Ok(())
}

/// Si `detailed`, remplace chaque élément par le résultat de `describe(id)` ;
/// sinon renvoie la liste telle quelle (cf. la boucle des commandes `list-*`).
fn collect<F>(items: Vec<Value>, detailed: bool, describe: F) -> Result<Vec<Value>>
where
    F: Fn(&str) -> Result<Value>,
{
    if !detailed {
        return Ok(items);
    }

    let mut out = Vec::with_capacity(items.len());
    for item in &items {
        match json_id(item) {
            Some(id) => out.push(describe(&id)?),
            None => out.push(item.clone()),
        }
    }
    Ok(out)
}

/// Imprime chaque élément, ou écrit toute la liste dans `filename` si `save`.
fn output_list(items: &[Value], save: bool, filename: &str) -> Result<()> {
    if save {
        fs::write(filename, serde_json::to_string_pretty(items)?)?;
        println!("Saved to {filename}");
    } else {
        for item in items {
            println!("{}", serde_json::to_string_pretty(item)?);
        }
    }
    Ok(())
}

/// Imprime un objet et, si `save`, l'écrit dans `filename` (façon `describe-*`).
fn output_single(item: &Value, save: bool, filename: &str) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(item)?);
    if save {
        fs::write(filename, serde_json::to_string_pretty(item)?)?;
        println!("Saved to {filename}");
    }
    Ok(())
}
