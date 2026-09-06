use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Challenge {
    pub name: String,
    pub description: String,
    pub signature: String,
    pub config: ChallengeConfig,
    pub tests: Vec<TestCase>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChallengeConfig {
    pub max_generations: u32,
    pub candidates_per_generation: u32,
    pub timeout_compile_secs: u64,
    pub timeout_run_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub input: String,
    pub expected: String,
}

pub struct LoadedChallenge {
    pub challenge: Challenge,
    pub reference_source: String,
}

/// Extract the function name from a signature like "fn solve(input: &[i64]) -> i64"
pub fn extract_func_name(signature: &str) -> String {
    signature
        .strip_prefix("fn ")
        .and_then(|s| s.split('(').next())
        .unwrap_or("solve")
        .trim()
        .to_string()
}

pub fn load(path: &Path) -> Result<LoadedChallenge> {
    let toml_path = path.join("challenge.toml");
    let ref_path = path.join("reference.rs");

    let toml_content = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("failed to read {}", toml_path.display()))?;
    let challenge: Challenge = toml::from_str(&toml_content)
        .with_context(|| format!("failed to parse {}", toml_path.display()))?;

    let reference_source = std::fs::read_to_string(&ref_path)
        .with_context(|| format!("failed to read {}", ref_path.display()))?;

    Ok(LoadedChallenge {
        challenge,
        reference_source,
    })
}
