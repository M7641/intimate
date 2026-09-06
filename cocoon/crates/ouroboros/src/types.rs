//! Types partagés du déploiement.

use std::path::PathBuf;

use serde::Serialize;

/// Décrit l'artefact à empaqueter et uploader : un chemin racine et la liste des
/// fichiers d'ignore (motifs gitwildmatch) à honorer. Équivalent du `TypedDict`
/// `Artifact` Python.
#[derive(Debug, Clone)]
pub struct Artifact {
    pub path: PathBuf,
    pub ignore_files: Vec<String>,
}

impl Default for Artifact {
    /// `{ path: ".", ignore_files: [".dockerignore"] }`, comme le défaut Python.
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            ignore_files: vec![".dockerignore".to_string()],
        }
    }
}

/// Argument de build Docker, sérialisé en `{"name": ..., "value": ...}`.
#[derive(Debug, Clone, Serialize)]
pub struct BuildArg {
    pub name: String,
    pub value: String,
}

impl BuildArg {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}
