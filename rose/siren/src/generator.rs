//! The image-modality counterpart to nyx's `LlmClient`.
//!
//! Image generation has fundamentally different inputs and outputs than chat
//! completion, so this lives in its own trait rather than overloading
//! `LlmClient`. Future modalities (audio, video) should follow the same rule:
//! one trait per modality, with its own option struct.

use image::RgbImage;

/// One round-trip request: prompt + sampling knobs.
///
/// `seed = None` means "fresh random seed each call" — that's what you want
/// during a reprompt session so each variation differs. Pass `Some(_)` to
/// reproduce a specific image, e.g. when re-rendering at a higher resolution.
#[derive(Debug, Clone)]
pub struct GenOptions {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub resolution: Resolution,
    pub steps: usize,
    pub guidance_scale: f64,
    pub seed: Option<u64>,
}

impl GenOptions {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            negative_prompt: None,
            // 512×512 is SD 1.5's training resolution — sharpest output, fastest path.
            resolution: Resolution::new(512, 512),
            steps: 30,
            guidance_scale: 7.5,
            seed: None,
        }
    }
}

/// Width × height. Stable Diffusion needs both divisible by 8 (latent space
/// is 1/8 of pixel space). We enforce it at construction so the pipeline
/// can trust the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: (width / 8) * 8,
            height: (height / 8) * 8,
        }
    }
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}×{}", self.width, self.height)
    }
}

/// A backend that turns text into an image.
///
/// Implementors so far: [`crate::candle_sd::StableDiffusionGenerator`] (local
/// Candle pipeline). The trait stays small on purpose — anything that's
/// pipeline-specific (scheduler choice, attention implementation) belongs in
/// the constructor of the concrete type, not here.
#[async_trait::async_trait]
pub trait ImageGenerator: Send + Sync {
    /// Human-readable backend name, shown in status lines.
    fn name(&self) -> &str;

    async fn generate(&self, opts: &GenOptions) -> anyhow::Result<RgbImage>;
}
