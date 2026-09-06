//! Assemble and write a reflection bundle to disk.
//!
//! Each run gets a timestamped directory holding `reflection.json` (the spec a
//! skill consumes) alongside the raw evidence (DOM, screenshots, and — later —
//! the HAR).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Local;
use serde::Serialize;

use crate::models::Reflection;

pub struct Bundle {
    pub dir: PathBuf,
}

fn run_id() -> String {
    Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Create a UI bundle `<out_root>/<timestamp>/` with the DOM + screenshot dirs.
pub fn new_bundle(out_root: &Path) -> Result<Bundle> {
    let dir = out_root.join(run_id());
    fs::create_dir_all(dir.join("dom"))?;
    fs::create_dir_all(dir.join("screenshots"))?;
    Ok(Bundle { dir })
}

/// Create an API bundle `<out_root>/<timestamp>/` with the samples dir.
pub fn new_api_bundle(out_root: &Path) -> Result<Bundle> {
    let dir = out_root.join(run_id());
    fs::create_dir_all(dir.join("samples"))?;
    Ok(Bundle { dir })
}

impl Bundle {
    /// Write a pretty-printed JSON file at the bundle root.
    pub fn write_json<T: Serialize>(&self, filename: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(value)?;
        fs::write(self.dir.join(filename), json)?;
        Ok(())
    }

    pub fn write_reflection(&self, reflection: &Reflection) -> Result<()> {
        self.write_json("reflection.json", reflection)
    }

    pub fn write_html(&self, html: &str) -> Result<()> {
        fs::write(self.dir.join("dom").join("initial.html"), html)?;
        Ok(())
    }

    pub fn screenshot_path(&self, name: &str) -> PathBuf {
        self.dir.join("screenshots").join(format!("{name}.png"))
    }

    /// Write one sample under `samples/`.
    pub fn write_sample<T: Serialize>(&self, filename: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(value)?;
        fs::write(self.dir.join("samples").join(filename), json)?;
        Ok(())
    }
}
