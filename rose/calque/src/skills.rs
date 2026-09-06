//! The consuming skills, bundled into the binary.
//!
//! The canonical source lives in `skills/<name>/SKILL.md` next to the crate and
//! is embedded at compile time, so `calque skills install` is self-contained —
//! it works from anywhere, without the repo present.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub struct BundledSkill {
    pub name: &'static str,
    pub skill_md: &'static str,
}

/// Skills shipped with this binary. Add a line here when a new skill is added
/// under `skills/`.
pub const SKILLS: &[BundledSkill] = &[
    BundledSkill {
        name: "reflect-to-react",
        skill_md: include_str!("../skills/reflect-to-react/SKILL.md"),
    },
    BundledSkill {
        name: "reflect-to-axum",
        skill_md: include_str!("../skills/reflect-to-axum/SKILL.md"),
    },
];

/// The user's personal skills directory: `~/.claude/skills`.
pub fn default_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".claude").join("skills"))
}

/// Write each bundled skill to `<target_dir>/<name>/SKILL.md`. Existing skills
/// are left untouched unless `force` is set. Returns the count written.
pub fn install(target_dir: &Path, force: bool) -> Result<usize> {
    let mut written = 0;
    for skill in SKILLS {
        let path = target_dir.join(skill.name).join("SKILL.md");
        if path.exists() && !force {
            println!("skip  {} (exists — use --force to overwrite)", skill.name);
            continue;
        }
        fs::create_dir_all(path.parent().expect("skill path has a parent"))?;
        fs::write(&path, skill.skill_md)?;
        println!("write {} → {}", skill.name, path.display());
        written += 1;
    }
    Ok(written)
}

/// Print the skills bundled in this binary.
pub fn list() {
    for skill in SKILLS {
        println!("{}", skill.name);
    }
}
