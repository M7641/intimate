use std::path::Path;

use crate::ui;

/// A file to write during scaffolding.
struct TemplateFile {
    /// Path relative to the destination root.
    path: &'static str,
    /// Raw content (embedded at compile time).
    content: &'static str,
    /// Whether to perform `{project_title}` / `{project_slug}` substitution.
    substitute: bool,
}

const FILES: &[TemplateFile] = &[
    TemplateFile {
        path: "package.json",
        content: include_str!("../../../templates/starlight/package.json"),
        substitute: true,
    },
    TemplateFile {
        path: "astro.config.mjs",
        content: include_str!("../../../templates/starlight/astro.config.mjs"),
        substitute: true,
    },
    TemplateFile {
        path: "tsconfig.json",
        content: include_str!("../../../templates/starlight/tsconfig.json"),
        substitute: false,
    },
    TemplateFile {
        path: ".gitignore",
        content: include_str!("../../../templates/starlight/.gitignore"),
        substitute: false,
    },
    TemplateFile {
        path: "src/content.config.ts",
        content: include_str!("../../../templates/starlight/src/content.config.ts"),
        substitute: false,
    },
    TemplateFile {
        path: "src/content/docs/index.mdx",
        content: include_str!("../../../templates/starlight/src/content/docs/index.mdx"),
        substitute: false,
    },
    TemplateFile {
        path: "src/content/docs/guides/example.md",
        content: include_str!("../../../templates/starlight/src/content/docs/guides/example.md"),
        substitute: false,
    },
    TemplateFile {
        path: "src/content/docs/reference/example.md",
        content: include_str!("../../../templates/starlight/src/content/docs/reference/example.md"),
        substitute: false,
    },
    TemplateFile {
        path: "src/components/Footer.astro",
        content: include_str!("../../../templates/starlight/src/components/Footer.astro"),
        substitute: false,
    },
    TemplateFile {
        path: "src/components/MarkdownContent.astro",
        content: include_str!("../../../templates/starlight/src/components/MarkdownContent.astro"),
        substitute: false,
    },
    TemplateFile {
        path: "src/components/Pagination.astro",
        content: include_str!("../../../templates/starlight/src/components/Pagination.astro"),
        substitute: false,
    },
    TemplateFile {
        path: "src/components/ThemeProvider.astro",
        content: include_str!("../../../templates/starlight/src/components/ThemeProvider.astro"),
        substitute: false,
    },
    TemplateFile {
        path: "src/components/ThemeSelect.astro",
        content: include_str!("../../../templates/starlight/src/components/ThemeSelect.astro"),
        substitute: false,
    },
    TemplateFile {
        path: "src/styles/custom.css",
        content: include_str!("../../../templates/starlight/src/styles/custom.css"),
        substitute: false,
    },
    TemplateFile {
        path: "src/styles/cyber.css",
        content: include_str!("../../../templates/starlight/src/styles/cyber.css"),
        substitute: false,
    },
    TemplateFile {
        path: "src/assets/.gitkeep",
        content: include_str!("../../../templates/starlight/src/assets/.gitkeep"),
        substitute: false,
    },
    TemplateFile {
        path: ".github/workflows/deploy.yml",
        content: include_str!("../../../templates/starlight/.github/workflows/deploy.yml"),
        substitute: false,
    },
];

/// Scaffold the Starlight template into `dest`, skipping files that already exist.
pub fn scaffold(dest: &Path, project_title: &str) -> anyhow::Result<()> {
    let project_slug = project_title.to_lowercase().replace(' ', "-");

    let mut written = 0u32;
    let mut skipped = 0u32;

    for file in FILES {
        let target = dest.join(file.path);

        if target.exists() {
            skipped += 1;
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = if file.substitute {
            file.content
                .replace("{project_title}", project_title)
                .replace("{project_slug}", &project_slug)
        } else {
            file.content.to_string()
        };

        std::fs::write(&target, &content)?;
        written += 1;
    }

    ui::status(&format!(
        "scaffolded {written} files ({skipped} already existed)"
    ));

    Ok(())
}
