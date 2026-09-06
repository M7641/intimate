//! Type d'erreur unique du crate.
//!
//! En Python, chaque méthode levait un `RuntimeError` formaté à la main après
//! avoir testé `response.status_code`. On remplace ça par une énumération typée :
//! le CLI peut afficher un message propre et la lib peut filtrer sur la variante.

/// Alias de confort : tout le crate renvoie `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// La variable d'environnement `API_KEY` est absente.
    #[error("API_KEY environment variable is not set")]
    MissingApiKey,

    /// Échec réseau / transport (DNS, TLS, timeout…).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// L'API a répondu avec un statut non attendu. On conserve le corps brut,
    /// comme le faisait le `f"{status} - {response.text}"` Python.
    #[error("Nimbus API error: {status} - {body}")]
    Api { status: u16, body: String },

    /// Type d'entité inconnu passé à `instance-options`.
    #[error("invalid entity type: {value}. Must be one of: {allowed}")]
    InvalidEntityType { value: String, allowed: String },

    /// Type d'image invalide passé à `create_image`.
    #[error("invalid image type: {value}. Must be one of: {allowed}")]
    InvalidImageType { value: String, allowed: String },

    /// Champ obligatoire manquant dans un corps de requête ou une réponse.
    #[error("missing required field: {0}")]
    MissingField(String),

    /// Le build de l'image a échoué côté plateforme.
    #[error("image build failed")]
    BuildFailed,

    /// Délai dépassé en attendant la fin du build de l'image.
    #[error("timed out waiting for image build")]
    BuildTimeout,

    /// Le chemin de l'artefact à zipper n'existe pas.
    #[error("artifact path does not exist: {0}")]
    ArtifactNotFound(String),

    /// Erreur de parcours du système de fichiers (collecte de l'artefact).
    #[error("filesystem walk error: {0}")]
    Walk(#[from] walkdir::Error),

    /// Erreur de compilation des motifs d'ignore (.dockerignore).
    #[error("ignore pattern error: {0}")]
    Ignore(#[from] ignore::Error),

    /// Erreur de création de l'archive zip.
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Erreur d'I/O (lecture de fichiers, écriture du `--save`, zip temporaire).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Sérialisation JSON impossible.
    #[error("failed to serialise JSON: {0}")]
    Json(#[from] serde_json::Error),
}
