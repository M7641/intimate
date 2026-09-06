mod template;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::llm::LlmClient;
use crate::ui;

const DOC_DIR: &str = "documentation";
const CONTENT_PATH: &str = "src/content/docs";

const PREAMBLE: &str = "\
You are a technical documentation writer. You generate clear, well-structured \
documentation in Markdown format for Starlight (Astro).

Rules:
- Write in a professional, concise tone
- Use proper Markdown headings (## for sections, ### for subsections)
- Include code examples where relevant
- Do NOT include frontmatter — it will be added automatically
- Do NOT wrap the output in markdown fences
- If the source code is insufficient to write a complete section, write what you \
can and add TODO comments for the gaps";

const MODEL: &str = "sonnet";

struct DocSection {
    name: &'static str,
    output_path: &'static str,
    source_patterns: &'static [&'static str],
    prompt_template: &'static str,
    frontmatter_title: &'static str,
    frontmatter_description: &'static str,
}

const DEFAULT_SECTIONS: &[DocSection] = &[
    DocSection {
        name: "Overview",
        output_path: "index.mdx",
        source_patterns: &["README*", "Cargo.toml", "pyproject.toml", "package.json"],
        prompt_template: "Write the landing page for this project's documentation. \
            Include: what the project does, who it's for, key features, \
            and a brief 'what's next' pointing to the Getting Started guide.",
        frontmatter_title: "Welcome",
        frontmatter_description: "Project documentation home page.",
    },
    DocSection {
        name: "Getting Started",
        output_path: "guides/getting-started.md",
        source_patterns: &[
            "README*",
            "Makefile",
            "Justfile",
            "Dockerfile",
            "docker-compose*",
            "*.toml",
            "*.lock",
        ],
        prompt_template: "Write a Getting Started guide. Cover: prerequisites, \
            installation, first run, and verifying it works. \
            Use numbered steps. Infer from the config/build files provided.",
        frontmatter_title: "Getting Started",
        frontmatter_description: "Install and run the project for the first time.",
    },
    DocSection {
        name: "Architecture",
        output_path: "guides/architecture.md",
        source_patterns: &[
            "src/**/*.rs",
            "src/**/*.py",
            "src/**/mod.rs",
            "src/lib.rs",
            "src/main.rs",
        ],
        prompt_template: "Write an Architecture Overview. Describe the high-level \
            structure: main modules/packages, how they relate, data flow, \
            and key design decisions you can infer from the code organization.",
        frontmatter_title: "Architecture",
        frontmatter_description: "High-level overview of the project structure.",
    },
    DocSection {
        name: "API Reference",
        output_path: "reference/api.md",
        source_patterns: &["src/**/*.rs", "src/**/*.py", "src/**/*.ts"],
        prompt_template: "Write an API Reference page. List public functions, \
            structs/classes, and their signatures. Group by module. \
            Include brief descriptions. Use tables or definition lists.",
        frontmatter_title: "API Reference",
        frontmatter_description: "Public interfaces and function signatures.",
    },
    DocSection {
        name: "Configuration",
        output_path: "reference/configuration.md",
        source_patterns: &[
            "*.toml",
            "*.yaml",
            "*.yml",
            "*.json",
            ".env.example",
            "*.cfg",
        ],
        prompt_template: "Write a Configuration Reference. Document every \
            configuration option you can find: environment variables, \
            config file fields, CLI flags. Use a table with columns: \
            Option, Type, Default, Description.",
        frontmatter_title: "Configuration",
        frontmatter_description: "Configuration options and environment variables.",
    },
    DocSection {
        name: "Contributing",
        output_path: "guides/contributing.md",
        source_patterns: &[
            "CONTRIBUTING*",
            "Makefile",
            "Justfile",
            ".github/**/*",
            "*.toml",
            "*.lock",
        ],
        prompt_template: "Write a Contributing guide. Cover: dev environment setup, \
            running tests, code style, branch/PR conventions, and CI checks. \
            Infer what you can from the build and CI files provided.",
        frontmatter_title: "Contributing",
        frontmatter_description: "How to set up a development environment and contribute.",
    },
];

/// Entry point for the docs agent.
pub async fn run(llm: Arc<dyn LlmClient>, title: Option<String>) -> anyhow::Result<()> {
    let doc_root = PathBuf::from(DOC_DIR);
    let content_root = doc_root.join(CONTENT_PATH);

    // Step 1-3: Scaffold if needed
    if !doc_root.exists() {
        ui::prompt("no documentation/ directory found. create one? [Y/n]");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "n" {
            ui::status("aborted");
            return Ok(());
        }

        let project_title = title.clone().unwrap_or_else(detect_project_name);

        ui::status(&format!(
            "scaffolding documentation for \"{project_title}\"..."
        ));
        template::scaffold(&doc_root, &project_title)?;
    } else {
        ui::status("documentation/ exists, checking for missing sections...");
    }

    // Step 4: Filter to missing sections only
    let missing: Vec<&DocSection> = DEFAULT_SECTIONS
        .iter()
        .filter(|s| !content_root.join(s.output_path).exists())
        .collect();

    if missing.is_empty() {
        ui::status("all sections already exist, nothing to generate");
        return Ok(());
    }

    ui::status(&format!(
        "{} missing section{} found",
        missing.len(),
        if missing.len() == 1 { "" } else { "s" },
    ));

    // Step 5-6: Generate one section at a time, asking before each
    let mut written = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    for section in &missing {
        ui::prompt(&format!(
            "generate \"{}\"? (→ {}) [Y/n/q(uit)]",
            section.name, section.output_path,
        ));
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        match input.trim().to_lowercase().as_str() {
            "q" | "quit" => {
                ui::status("stopped");
                break;
            }
            "n" | "no" => {
                skipped += 1;
                continue;
            }
            _ => {} // "y", "yes", or empty → proceed
        }

        ui::status(&format!("generating {}...", section.name));

        let context = gather_context(
            &section
                .source_patterns
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        );

        if context.is_empty() {
            ui::status(&format!(
                "skipped {} — no source files found for context",
                section.name
            ));
            skipped += 1;
            continue;
        }

        let prompt = format!(
            "{}\n\nHere is the project context:\n\n{context}",
            section.prompt_template,
        );

        match llm.prompt_with_model(PREAMBLE, &prompt, Some(MODEL)).await {
            Ok(response) => {
                let full = format!(
                    "---\ntitle: {}\ndescription: {}\n---\n\n{}",
                    section.frontmatter_title,
                    section.frontmatter_description,
                    response.trim(),
                );
                let target = content_root.join(section.output_path);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, &full)?;
                ui::status(&format!("wrote {}", section.output_path));
                written += 1;
            }
            Err(e) => {
                ui::status(&format!("failed: {} — {e}", section.name));
                failed += 1;
            }
        }
    }

    // Step 7: Summary
    ui::status(&format!(
        "done: {written} generated, {skipped} skipped, {failed} failed",
    ));

    if written > 0 {
        ui::status("update sidebar in documentation/astro.config.mjs if needed");
    }

    Ok(())
}

/// Try to detect the project name from config files, falling back to the directory name.
fn detect_project_name() -> String {
    // Try Cargo.toml
    if let Ok(content) = std::fs::read_to_string("Cargo.toml")
        && let Some(name) = extract_toml_name(&content)
    {
        return titlecase(&name);
    }

    // Try pyproject.toml
    if let Ok(content) = std::fs::read_to_string("pyproject.toml")
        && let Some(name) = extract_toml_name(&content)
    {
        return titlecase(&name);
    }

    // Try package.json
    if let Ok(content) = std::fs::read_to_string("package.json")
        && let Some(name) = extract_json_name(&content)
    {
        return titlecase(&name);
    }

    // Fallback: current directory name
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| titlecase(&n.to_string_lossy())))
        .unwrap_or_else(|| "My Docs".to_string())
}

/// Rough extraction of `name = "..."` from a TOML file (avoids adding a toml parser dep).
fn extract_toml_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name")
            && let Some(val) = trimmed.split('=').nth(1)
        {
            let val = val.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Rough extraction of `"name": "..."` from a JSON file.
fn extract_json_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"name\"")
            && let Some(val) = trimmed.split(':').nth(1)
        {
            let val = val.trim().trim_matches(',').trim().trim_matches('"');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Convert "my-project" or "my_project" to "My Project".
fn titlecase(s: &str) -> String {
    s.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collect file contents matching the given patterns from the current directory.
///
/// Patterns supported:
/// - `"README*"` — prefix match on filename
/// - `"*.toml"` — extension match
/// - `"src/**/*.rs"` — directory prefix + extension match
///
/// Reads up to `CONTEXT_BUDGET` bytes total.
const CONTEXT_BUDGET: usize = 50 * 1024; // 50 KB

const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
    "documentation",
];

fn gather_context(patterns: &[String]) -> String {
    let mut files: Vec<PathBuf> = Vec::new();
    walk_for_context(Path::new("."), &mut files);

    let mut parts = Vec::new();
    let mut budget = CONTEXT_BUDGET;

    for file in &files {
        let display = file
            .to_string_lossy()
            .strip_prefix("./")
            .unwrap_or(&file.to_string_lossy())
            .to_string();

        if !matches_any_pattern(&display, patterns) {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(file) else {
            continue;
        };

        if content.len() > budget {
            // Take what we can
            let truncated = &content[..budget];
            parts.push(format!("--- {display} (truncated) ---\n{truncated}"));
            break;
        }

        budget -= content.len();
        parts.push(format!("--- {display} ---\n{content}"));

        if budget == 0 {
            break;
        }
    }

    parts.join("\n\n")
}

fn walk_for_context(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') && name_str != ".env.example" {
            continue;
        }

        if path.is_dir() {
            if SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            walk_for_context(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Check if a file path matches any of the source patterns.
fn matches_any_pattern(path: &str, patterns: &[String]) -> bool {
    let filename = Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    for pattern in patterns {
        if pattern.contains("**/") {
            // "src/**/*.rs" → check dir prefix + extension
            let parts: Vec<&str> = pattern.splitn(2, "**/").collect();
            if parts.len() == 2 {
                let dir_prefix = parts[0]; // e.g. "src/"
                let file_pattern = parts[1]; // e.g. "*.rs"

                if (path.starts_with(dir_prefix) || dir_prefix.is_empty())
                    && matches_simple(file_pattern, &filename)
                {
                    return true;
                }
            }
        } else if pattern.contains('*') {
            // "*.toml" or "README*"
            if matches_simple(pattern, &filename) {
                return true;
            }
        } else {
            // Exact match on filename
            if filename == *pattern {
                return true;
            }
        }
    }

    false
}

/// Simple glob: `"*.rs"` or `"README*"`.
fn matches_simple(pattern: &str, name: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        name == pattern
    }
}
