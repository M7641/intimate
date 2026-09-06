//! Full-page screenshots (verbatim from calque), used as finding evidence.

use std::path::Path;

use anyhow::Result;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;

pub async fn full_page(page: &Page, path: &Path) -> Result<()> {
    let params = ScreenshotParams::builder().full_page(true).build();
    page.save_screenshot(params, path).await?;
    Ok(())
}
