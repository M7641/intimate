//! Sortie terminale stylée, partagée par les CLIs des apps (`data_view`,
//! `warehouse`).
//!
//! Extrait du code dupliqué des `cli.rs` des deux apps. Les styles sont des
//! constantes `anstyle` (séquence ANSI via `Display`, reset via le flag `{:#}`),
//! et l'affichage passe par `anstream::println!`/`eprintln!` qui **stripent
//! automatiquement les couleurs hors TTY** (sortie redirigée, CI, logs).

use std::path::Path;

use anstyle::{AnsiColor, Color, Style};

const fn fg(color: AnsiColor) -> Style {
    Style::new().fg_color(Some(Color::Ansi(color)))
}

const ACCENT: Style = fg(AnsiColor::BrightCyan);
const ACCENT_BOLD: Style = fg(AnsiColor::BrightCyan).bold();
const OK: Style = fg(AnsiColor::BrightGreen).bold();
const ERR: Style = fg(AnsiColor::BrightRed).bold();
const DIM: Style = Style::new().dimmed();
const BOLD: Style = Style::new().bold();

/// En-tête d'une commande : `◆  <app>  <sous-titre>`.
pub fn header(app: &str, subtitle: &str) {
    anstream::println!();
    anstream::println!(
        "  {ACCENT_BOLD}◆{ACCENT_BOLD:#}  {BOLD}{app}{BOLD:#}  {DIM}{subtitle}{DIM:#}"
    );
}

/// Étape : `▸ <commande>  (<cwd>)`, le répertoire en grisé.
pub fn step(command: &str, cwd: &Path) {
    anstream::println!(
        "  {ACCENT}▸{ACCENT:#} {command}  {DIM}({}){DIM:#}",
        cwd.display()
    );
}

/// Ligne d'information discrète (grisée).
pub fn note(message: &str) {
    anstream::println!("  {DIM}{message}{DIM:#}");
}

/// Succès : `✓ <message>` en vert.
pub fn success(message: &str) {
    anstream::println!("  {OK}✓{OK:#} {message}");
}

/// Erreur : `✗ <message>` en rouge, sur stderr.
pub fn error(message: &str) {
    anstream::eprintln!("  {ERR}✗{ERR:#} {message}");
}

/// Logging minimal pour les commandes CLI : fait remonter les évènements
/// `tracing` des librairies (ex. la progression de déploiement d'`ouroboros`)
/// sous forme de lignes épurées (sans niveau, cible ni horodatage).
///
/// Sans erreur si un subscriber global est déjà installé.
pub fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(false)
        .without_time()
        .try_init();
}
