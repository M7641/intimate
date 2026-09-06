//! Interactive reprompt loop.
//!
//! Flow:
//!
//!   ┌──────────────────────────────────┐
//!   │  generate at preview resolution   │
//!   │  open in viewer                   │
//!   └──────────┬───────────────────────┘
//!              ▼
//!      [r]eprompt → loop
//!      [v]ary    → same prompt, new seed → loop
//!      [s]ave    → ask resolution → re-render at target res (same seed)
//!                  → preview → confirm → write to output dir → exit
//!      [q]uit    → discard, exit

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use image::RgbImage;

use crate::generator::{GenOptions, ImageGenerator, Resolution};
use crate::{ui, viewer};

pub struct Session<G: ImageGenerator> {
    generator: G,
    output_dir: PathBuf,
    /// Persisted across iterations so the user can riff: change prompt but
    /// keep steps/cfg/etc. Reset to a fresh random seed each round unless
    /// the user "saves" — saving locks the seed for the high-res re-render.
    opts: GenOptions,
    iteration: u32,
}

impl<G: ImageGenerator> Session<G> {
    pub fn new(generator: G, output_dir: PathBuf, initial_prompt: String) -> Self {
        Self {
            generator,
            output_dir,
            opts: GenOptions::new(initial_prompt),
            iteration: 0,
        }
    }

    pub fn set_steps(&mut self, steps: usize) {
        self.opts.steps = steps;
    }

    pub fn set_guidance(&mut self, guidance: f64) {
        self.opts.guidance_scale = guidance;
    }

    pub async fn run(mut self) -> Result<()> {
        ui::status(&format!("backend: {}", self.generator.name()));

        loop {
            self.iteration += 1;
            // Fresh seed for variation each round, unless caller pinned one.
            // We *remember* the seed used so [s]ave can replay it at hi-res.
            let seed = self.opts.seed.unwrap_or_else(rand::random);
            self.opts.seed = Some(seed);

            let img = self.generator.generate(&self.opts).await?;
            let preview_path = viewer::write_temp_preview(&img, self.iteration)?;
            viewer::open_in_viewer(&preview_path)?;
            ui::success(&format!(
                "preview #{}: {}",
                self.iteration,
                preview_path.display()
            ));

            match self.menu()? {
                Action::Reprompt(new) => {
                    self.opts.prompt = new;
                    self.opts.seed = None; // fresh seed with new prompt
                }
                Action::Vary => {
                    self.opts.seed = None; // re-randomize next round
                }
                Action::Save => {
                    if let Some(path) = self.save_flow(&img).await? {
                        ui::success(&format!("saved: {}", path.display()));
                        return Ok(());
                    }
                    // user cancelled the save dialog — fall back to the menu
                }
                Action::Quit => {
                    ui::status("discarded, exiting");
                    return Ok(());
                }
            }
        }
    }

    fn menu(&self) -> Result<Action> {
        eprintln!();
        eprintln!("    [r]eprompt   [v]ary (new seed)   [s]ave   [q]uit");
        let choice = ui::read_choice("?", 's')?;
        match choice {
            'r' => {
                let new = ui::read_line("new prompt:")?.unwrap_or_default();
                if new.is_empty() {
                    ui::warn("empty prompt, keeping previous");
                    Ok(Action::Vary)
                } else {
                    Ok(Action::Reprompt(new))
                }
            }
            'v' => Ok(Action::Vary),
            'q' => Ok(Action::Quit),
            _ => Ok(Action::Save),
        }
    }

    /// Save flow with resolution choice + user-verified re-render.
    /// Returns the saved path, or `None` if the user cancelled.
    async fn save_flow(&mut self, preview_img: &RgbImage) -> Result<Option<PathBuf>> {
        loop {
            let res = match self.ask_resolution()? {
                Some(r) => r,
                None => return Ok(None),
            };

            // If user picked the same resolution we already have, skip the
            // re-render — preview_img is already at that res.
            let same = res.width == self.opts.resolution.width
                && res.height == self.opts.resolution.height;

            let final_img = if same {
                ui::status("using existing render");
                preview_img.clone()
            } else {
                ui::status(&format!(
                    "re-rendering at {} (same seed for consistency)",
                    res
                ));
                let mut hi = self.opts.clone();
                hi.resolution = res;
                // seed already pinned from the run loop above
                self.generator.generate(&hi).await?
            };

            // Show the would-be saved file before committing to disk.
            let confirm_path = viewer::write_temp_preview(&final_img, 999)?;
            viewer::open_in_viewer(&confirm_path)?;
            ui::status(&format!("verify preview: {}", confirm_path.display()));

            let choice = ui::read_choice("save this? [Y/n/r=pick another resolution]", 'y')?;
            match choice {
                'n' => return Ok(None),
                'r' => continue,
                _ => {
                    let final_path = self.write_to_output_dir(&final_img, res)?;
                    return Ok(Some(final_path));
                }
            }
        }
    }

    fn ask_resolution(&self) -> Result<Option<Resolution>> {
        eprintln!();
        eprintln!("    resolution presets:");
        eprintln!("      [1] 512×512   (preview / SD-1.5 native)");
        eprintln!("      [2] 768×768");
        eprintln!("      [3] 1024×1024");
        eprintln!("      [4] 1024×768  (landscape)");
        eprintln!("      [5] 768×1024  (portrait)");
        eprintln!("      [c] custom WIDTHxHEIGHT");
        eprintln!("      [q] cancel");
        let choice = ui::read_choice("resolution?", '1')?;
        let res = match choice {
            '1' => Resolution::new(512, 512),
            '2' => Resolution::new(768, 768),
            '3' => Resolution::new(1024, 1024),
            '4' => Resolution::new(1024, 768),
            '5' => Resolution::new(768, 1024),
            'c' => {
                let s = ui::read_line("WIDTHxHEIGHT:")?.unwrap_or_default();
                match parse_resolution(&s) {
                    Some(r) => r,
                    None => {
                        ui::warn(&format!("couldn't parse '{s}', cancelling"));
                        return Ok(None);
                    }
                }
            }
            'q' => return Ok(None),
            _ => Resolution::new(512, 512),
        };
        Ok(Some(res))
    }

    fn write_to_output_dir(&self, img: &RgbImage, res: Resolution) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.output_dir)?;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let slug = slugify(&self.opts.prompt, 40);
        let filename = format!("siren-{ts}-{}x{}-{slug}.png", res.width, res.height);
        let path = self.output_dir.join(filename);
        img.save(&path)?;
        Ok(path)
    }
}

enum Action {
    Reprompt(String),
    Vary,
    Save,
    Quit,
}

fn parse_resolution(s: &str) -> Option<Resolution> {
    // Accept "1024x768", "1024×768", "1024 768"
    let cleaned: String = s
        .chars()
        .map(|c| if c == '×' || c == ' ' { 'x' } else { c })
        .collect();
    let (w, h) = cleaned.split_once('x')?;
    let w: u32 = w.trim().parse().ok()?;
    let h: u32 = h.trim().parse().ok()?;
    Some(Resolution::new(w, h))
}

/// Make a prompt safe for a filename: lowercase, alphanumerics + dashes only.
fn slugify(s: &str, max_len: usize) -> String {
    let slug: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    let mut out = String::new();
    let mut prev_dash = false;
    for c in slug.chars().take(max_len) {
        if c == '-' {
            if !prev_dash {
                out.push('-');
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    out.trim_matches('-').to_string()
}
