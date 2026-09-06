use crate::ui;
use std::path::Path;

const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
];

const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "go", "rb", "java", "c", "cpp", "h", "hpp", "cs",
    "swift", "kt", "lua", "sh", "bash", "zsh", "toml", "yaml", "yml", "json", "sql", "html", "css",
    "scss", "vue", "svelte", "zig", "hs", "ml", "ex", "exs", "clj", "R", "jl",
];

/// Recursively collect source files from `root`, skipping hidden and build dirs.
pub fn collect_files(root: &str) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();
    walk_dir(Path::new(root), &mut files)?;
    files.sort();
    Ok(files)
}

/// Display an interactive file picker and let the user select files.
///
/// `max_select` caps how many files can be chosen (0 = unlimited).
pub fn pick_files_with_limit(max_select: usize) -> anyhow::Result<Vec<String>> {
    let entries = collect_files(".")?;

    if entries.is_empty() {
        anyhow::bail!("no source files found in current directory");
    }

    ui::status(&format!("found {} files", entries.len()));

    let indices = ui::pick_files_interactive(&entries, max_select)
        .map_err(|e| anyhow::anyhow!("picker failed: {e}"))?;

    if indices.is_empty() {
        anyhow::bail!("no files selected");
    }

    let selected: Vec<String> = indices.into_iter().map(|i| entries[i].clone()).collect();
    Ok(selected)
}

fn walk_dir(dir: &Path, out: &mut Vec<String>) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            if SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            walk_dir(&path, out)?;
        } else if is_source_file(&name_str) {
            let display = path
                .to_string_lossy()
                .strip_prefix("./")
                .unwrap_or(&path.to_string_lossy())
                .to_string();
            out.push(display);
        }
    }
    Ok(())
}

fn is_source_file(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .map(|ext| SOURCE_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}
