//! Construction de l'artefact de déploiement : collecte des fichiers en honorant
//! les motifs `.dockerignore`, puis archivage zip.
//!
//! Le Python utilisait `pathspec` (gitwildmatch) + `os.walk` + `zipfile`. On
//! reproduit ça avec `ignore` (le moteur gitignore de ripgrep) pour les motifs,
//! `walkdir` pour le parcours, et `zip` pour l'archive.

use std::io::Write;
use std::path::{Path, PathBuf};

use ignore::gitignore::GitignoreBuilder;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

use crate::error::{Error, Result};

/// Liste les chemins (relatifs à `path`) à inclure, après filtrage par les motifs
/// lus dans les `ignore_files` présents.
///
/// Les dossiers ignorés sont **élagués** (non parcourus) — indispensable pour ne
/// pas descendre dans `node_modules`/`target` exclus par `.dockerignore`.
pub(crate) fn get_files_to_include(path: &Path, ignore_files: &[String]) -> Result<Vec<PathBuf>> {
    let mut builder = GitignoreBuilder::new(path);
    for ignore_file in ignore_files {
        let ignore_path = path.join(ignore_file);
        if ignore_path.exists() {
            // `add` renvoie une erreur de parsing non fatale : on l'ignore comme
            // le faisait le best-effort Python.
            let _ = builder.add(ignore_path);
        }
    }
    let matcher = builder.build()?;

    let mut files = Vec::new();
    let walker = WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| {
            let Ok(rel) = entry.path().strip_prefix(path) else {
                return true;
            };
            if rel.as_os_str().is_empty() {
                return true; // la racine elle-même
            }
            let is_dir = entry.file_type().is_dir();
            !matcher.matched_path_or_any_parents(rel, is_dir).is_ignore()
        });

    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_file()
            && let Ok(rel) = entry.path().strip_prefix(path)
        {
            files.push(rel.to_path_buf());
        }
    }
    Ok(files)
}

/// Écrit l'archive zip de l'artefact dans `zip_path` (DEFLATE, niveau max).
///
/// Si `artifact_path` est un fichier, il doit s'agir d'un Dockerfile (parité avec
/// le Python) ; sinon on archive tous les fichiers retenus par [`get_files_to_include`].
pub(crate) fn build_zip(
    artifact_path: &Path,
    zip_path: &Path,
    ignore_files: &[String],
) -> Result<()> {
    if !artifact_path.exists() {
        return Err(Error::ArtifactNotFound(artifact_path.display().to_string()));
    }

    let file = std::fs::File::create(zip_path)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9));

    if artifact_path.is_file() {
        let name = artifact_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Dockerfile");
        writer.start_file(name, options)?;
        writer.write_all(&std::fs::read(artifact_path)?)?;
    } else {
        for rel in get_files_to_include(artifact_path, ignore_files)? {
            let full = artifact_path.join(&rel);
            if full.is_file() {
                // Les entrées zip utilisent toujours '/' comme séparateur.
                let name = rel.to_string_lossy().replace('\\', "/");
                writer.start_file(name, options)?;
                writer.write_all(&std::fs::read(&full)?)?;
            }
        }
    }

    writer.finish()?;
    Ok(())
}
