//! Client de lecture pour l'API quota (options d'instances) de Nimbus.
//!
//! Porte aussi le rendu en table de `instance-options`, équivalent du
//! `rich.Table` Python via [`comfy_table`].

use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, Color, Table};
use serde_json::Value;

use crate::client::{Client, array_field};
use crate::error::{Error, Result};

const BASE_URL: &str = "https://service.nimbus.example/quota/api/v1";

/// Types d'entité acceptés par l'endpoint quota (cf. nimbus-sdk).
const ENTITY_TYPES: &[&str] = &[
    "api-deployment",
    "dataBridge-dataCatalog",
    "dataBridge-dataLake",
    "dataBridge-dataWareHouse",
    "feed",
    "webapp",
    "workflow",
    "workspace",
    "pat",
    "sat",
];

/// Vue « tenant / quota » au-dessus du [`Client`] partagé.
pub struct Tenant<'a> {
    client: &'a Client,
}

impl<'a> Tenant<'a> {
    #[must_use]
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Récupère les options d'instances pour un type d'entité donné.
    ///
    /// # Errors
    /// [`Error::InvalidEntityType`] si `entity_type` n'est pas reconnu, sinon
    /// toute erreur HTTP ou API.
    pub fn instance_options(&self, entity_type: &str) -> Result<Vec<Value>> {
        if !ENTITY_TYPES.contains(&entity_type) {
            return Err(Error::InvalidEntityType {
                value: entity_type.to_string(),
                allowed: ENTITY_TYPES.join(", "),
            });
        }

        let body = self.client.get_json(
            &format!("{BASE_URL}/settings/tenant-instance-options"),
            &[("entityType", entity_type)],
        )?;
        Ok(array_field(&body, "data"))
    }

    /// Affiche les options d'instances groupées par classe, façon tableau.
    ///
    /// # Errors
    /// Identique à [`Tenant::instance_options`].
    pub fn print_instance_options(&self, entity_type: &str) -> Result<()> {
        let instances = self.instance_options(entity_type)?;

        if instances.is_empty() {
            println!("No instance options found for {entity_type}");
            return Ok(());
        }

        // Groupage par `instanceClass` en préservant l'ordre de première apparition
        // (comme un dict Python), sans dépendre d'un ordre alphabétique.
        let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
        for inst in &instances {
            let class = get_str(inst, "instanceClass", "Unknown").to_string();
            match groups.iter_mut().find(|(c, _)| *c == class) {
                Some((_, group)) => group.push(inst),
                None => groups.push((class, vec![inst])),
            }
        }

        // Au sein d'une classe, tri par CPU croissant.
        for (_, group) in &mut groups {
            group.sort_by(|a, b| {
                get_f64(a, "cpu")
                    .partial_cmp(&get_f64(b, "cpu"))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // Colonnes GPU affichées seulement si au moins une instance en a.
        let has_gpu = instances.iter().any(|i| is_truthy(i, "gpu"));

        for (class, group) in &groups {
            println!();
            println!("  {class}");
            println!("{}", render_table(group, has_gpu));
        }

        println!();
        println!(
            "  {} instance types available for {entity_type}",
            instances.len()
        );
        println!(
            "  *Estimated daily cost based on Fargate pricing \
             ($0.04/vCPU/hr + $0.005/GB/hr, 24h)"
        );
        Ok(())
    }
}

/// Construit le tableau d'une classe d'instances.
fn render_table(group: &[&Value], has_gpu: bool) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);

    let mut header: Vec<Cell> = vec![
        header_cell("ID"),
        header_cell("Name"),
        header_cell("CPU"),
        header_cell("Memory"),
    ];
    if has_gpu {
        header.push(header_cell("GPU"));
        header.push(header_cell("GPU Mem"));
    }
    header.push(header_cell("Daily Cost*"));
    header.push(header_cell("Provider"));
    table.set_header(header);

    for inst in group {
        let cpu_val = get_f64(inst, "cpu");
        let mem_val = get_f64(inst, "memory");
        let vcpu = cpu_val / 1000.0;
        let gb = mem_val / 1000.0;

        let cpu = if cpu_val > 0.0 {
            format!("{vcpu} vCPU")
        } else {
            "—".to_string()
        };
        let mem = if mem_val > 0.0 {
            format!("{gb} GB")
        } else {
            "—".to_string()
        };

        let daily_cost = 24.0 * (vcpu * 0.04 + gb * 0.005);
        let cost = if daily_cost > 0.0 {
            format!("${daily_cost:.2}")
        } else {
            "—".to_string()
        };

        // Le nom Nimbus inclut souvent une parenthèse descriptive : on ne garde
        // que la partie avant '(' (équivalent du `.split("(")[0].strip()`).
        let raw_name = get_str(inst, "name", "");
        let name = raw_name.split('(').next().unwrap_or(raw_name).trim();

        let id = inst
            .get("id")
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();

        let mut row: Vec<Cell> = vec![
            Cell::new(id).fg(Color::DarkGrey),
            Cell::new(name),
            Cell::new(cpu).fg(Color::Green),
            Cell::new(mem).fg(Color::Blue),
        ];

        if has_gpu {
            let gpu = if is_truthy(inst, "gpu") {
                get_f64(inst, "gpu").to_string()
            } else {
                "—".to_string()
            };
            let gpu_mem = if is_truthy(inst, "gpuMemory") {
                format!("{} GB", get_f64(inst, "gpuMemory"))
            } else {
                "—".to_string()
            };
            row.push(Cell::new(gpu).fg(Color::Yellow));
            row.push(Cell::new(gpu_mem).fg(Color::Yellow));
        }

        row.push(
            Cell::new(cost)
                .fg(Color::Yellow)
                .add_attribute(Attribute::Bold),
        );
        row.push(Cell::new(get_str(inst, "provider", "—")).fg(Color::DarkGrey));
        table.add_row(row);
    }

    table
}

fn header_cell(text: &str) -> Cell {
    Cell::new(text)
        .fg(Color::Magenta)
        .add_attribute(Attribute::Bold)
}

/// Lit un champ numérique (`0.0` par défaut), tolérant aux `null`.
fn get_f64(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Lit un champ chaîne, avec valeur de repli.
fn get_str<'v>(value: &'v Value, key: &str, default: &'v str) -> &'v str {
    value.get(key).and_then(Value::as_str).unwrap_or(default)
}

/// Vrai si le champ existe et n'est ni `null`, ni `0`, ni `""` — reproduit la
/// véracité Python d'un `inst.get(key)`.
fn is_truthy(value: &Value, key: &str) -> bool {
    match value.get(key) {
        None | Some(Value::Null) => false,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Bool(b)) => *b,
        Some(_) => true,
    }
}
