//! Screenshots. v1 captures the full initial page; per-interaction-state shots
//! arrive with the recipe engine, and become visual-regression baselines.

use std::path::Path;

use anyhow::Result;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;

pub async fn full_page(page: &Page, path: &Path) -> Result<()> {
    let params = ScreenshotParams::builder().full_page(true).build();
    page.save_screenshot(params, path).await?;
    Ok(())
}
