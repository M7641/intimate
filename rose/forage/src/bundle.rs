//! The output bundle — a timestamped run directory (adapted from calque's
//! `reflect.rs`). Holds the reports and an `evidence/` dir for screenshots.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Local;
use serde::Serialize;

pub struct Bundle {
    pub dir: PathBuf,
}

fn run_id() -> String {
    Local::now().format("%Y%m%d-%H%M%S").to_string()
}

pub fn new_bundle(out_root: &Path) -> Result<Bundle> {
    let dir = out_root.join(run_id());
    fs::create_dir_all(dir.join("evidence"))?;
    Ok(Bundle { dir })
}

impl Bundle {
    pub fn write_json<T: Serialize>(&self, filename: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(value)?;
        fs::write(self.dir.join(filename), json)?;
        Ok(())
    }

    pub fn write_text(&self, filename: &str, text: &str) -> Result<()> {
        fs::write(self.dir.join(filename), text)?;
        Ok(())
    }

    /// Path for an evidence screenshot; returns the relative name to store on
    /// the finding too.
    pub fn evidence_path(&self, name: &str) -> (PathBuf, String) {
        let file = format!("{name}.png");
        (
            self.dir.join("evidence").join(&file),
            format!("evidence/{file}"),
        )
    }
}
