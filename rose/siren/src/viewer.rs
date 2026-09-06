//! Open an image in the OS's default viewer.
//!
//! We write the image to a temp file (so the viewer reads a real path, not a
//! pipe) and shell out to `open` on macOS. This is the simplest reliable
//! preview path — terminal-inline previews via sixel/kitty graphics work but
//! depend on the terminal emulator, and we'd rather not have a "your image
//! looks like garbage" failure mode during a creative session.

use std::path::{Path, PathBuf};

use anyhow::Context;
use image::RgbImage;

/// Persist `img` to a uniquely-named PNG in the OS temp dir and return the path.
/// Each iteration gets its own file so the user can scroll back through them
/// if their viewer supports it (Preview.app does).
pub fn write_temp_preview(img: &RgbImage, iteration: u32) -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join("siren");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "preview-{}-{:03}.png",
        std::process::id(),
        iteration
    ));
    img.save(&path)
        .with_context(|| format!("failed to write preview PNG to {}", path.display()))?;
    Ok(path)
}

/// Open `path` in the default viewer. On macOS this is `open <path>`, which
/// hands the file off to Preview.app (or whichever app is bound to .png).
pub fn open_in_viewer(path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(target_os = "linux")]
    let program = "xdg-open";
    #[cfg(target_os = "windows")]
    let program = "explorer";

    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .with_context(|| format!("failed to launch image viewer ({program})"))?;
    Ok(())
}
