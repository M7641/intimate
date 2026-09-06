//! Stable Diffusion 1.5 pipeline using `candle-transformers`.
//!
//! Phases (in order, named so a future contributor can splice in img2img,
//! ControlNet, LoRA, etc. without untangling the whole function):
//!
//!   1. tokenize_prompts      — CLIP tokenizer, prompt + negative
//!   2. encode_text           — CLIP text transformer → embeddings
//!   3. sample_initial_latents — Gaussian noise scaled by scheduler sigma
//!   4. denoise_loop          — UNet + CFG, scheduler.step per timestep
//!   5. decode_latents        — VAE decoder → RGB image tensor

use std::path::PathBuf;

use anyhow::{Context, Result};
use candle_core::{DType, Device, IndexOp, Module, Tensor};
use candle_transformers::models::stable_diffusion::{
    self, clip::ClipTextTransformer, unet_2d::UNet2DConditionModel, vae::AutoEncoderKL,
    StableDiffusionConfig,
};
use hf_hub::api::tokio::Api;
use image::RgbImage;
use tokenizers::Tokenizer;

use crate::generator::{GenOptions, ImageGenerator};
use crate::ui;

/// Repo holding SD 1.5 unet + vae weights. The original `runwayml/...` repo
/// was unpublished by the authors; this is the canonical community mirror.
const SD15_REPO: &str = "stable-diffusion-v1-5/stable-diffusion-v1-5";
/// CLIP ViT-L/14 — SD 1.5's text encoder is sourced from this repo.
const CLIP_REPO: &str = "openai/clip-vit-large-patch14";

/// Scaling factor between VAE latent space and image pixel space.
/// Hard-coded in the SD 1.5 VAE — DON'T change it.
const VAE_SCALE: f64 = 0.18215;

pub struct StableDiffusionGenerator {
    config: StableDiffusionConfig,
    tokenizer: Tokenizer,
    clip: ClipTextTransformer,
    unet: UNet2DConditionModel,
    vae: AutoEncoderKL,
    device: Device,
    dtype: DType,
}

impl StableDiffusionGenerator {
    /// Download weights (cached via hf-hub) and instantiate the four models.
    /// First run pulls ~5 GB; subsequent runs are instant.
    pub async fn load() -> Result<Self> {
        let device = select_device()?;
        // f16 on GPU ≈ 2× speed and half the memory vs f32, with no visible
        // quality loss at SD 1.5's scale. Stays f32 on CPU because Candle's
        // CPU f16 paths are slow.
        let dtype = if device.is_cpu() {
            DType::F32
        } else {
            DType::F16
        };
        ui::status(&format!("device={:?} dtype={:?}", device, dtype));

        let api = Api::new()?;
        let sd_repo = api.model(SD15_REPO.to_string());
        let clip_repo = api.model(CLIP_REPO.to_string());

        ui::status("downloading model weights (cached after first run)...");
        let (tokenizer_path, clip_weights, unet_weights, vae_weights) = tokio::try_join!(
            fetch(&clip_repo, "tokenizer.json"),
            fetch(&clip_repo, "model.safetensors"),
            fetch(&sd_repo, "unet/diffusion_pytorch_model.fp16.safetensors"),
            fetch(&sd_repo, "vae/diffusion_pytorch_model.fp16.safetensors"),
        )?;

        // Resolution is set per-request via build_with(); we pass a default
        // here just to satisfy the constructor.
        let config = StableDiffusionConfig::v1_5(None, Some(512), Some(512));

        ui::status("loading CLIP text encoder...");
        // `build_clip_transformer` is a free function in 0.9 — it takes the
        // CLIP config explicitly so SDXL (which has two text encoders) can
        // reuse it. For SD 1.5 we just pass `config.clip`.
        let clip =
            stable_diffusion::build_clip_transformer(&config.clip, &clip_weights, &device, dtype)?;

        ui::status("loading UNet...");
        let unet = config.build_unet(&unet_weights, &device, 4, false, dtype)?;

        ui::status("loading VAE...");
        let vae = config.build_vae(&vae_weights, &device, dtype)?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("tokenizer load failed: {e}"))?;

        ui::status("pipeline ready");
        Ok(Self {
            config,
            tokenizer,
            clip,
            unet,
            vae,
            device,
            dtype,
        })
    }

    // ── Phase 1: tokenize ────────────────────────────────────────────────
    fn tokenize_prompts(&self, prompt: &str, negative: &str) -> Result<(Tensor, Tensor)> {
        let max_len = self.config.clip.max_position_embeddings;
        let pad_id = *self
            .tokenizer
            .get_vocab(true)
            .get("<|endoftext|>")
            .context("CLIP tokenizer missing <|endoftext|>")? as i64 as u32;

        let encode = |text: &str| -> Result<Tensor> {
            let mut ids = self
                .tokenizer
                .encode(text, true)
                .map_err(|e| anyhow::anyhow!("tokenize failed: {e}"))?
                .get_ids()
                .to_vec();
            ids.resize(max_len, pad_id);
            Ok(Tensor::new(ids.as_slice(), &self.device)?.unsqueeze(0)?)
        };

        Ok((encode(prompt)?, encode(negative)?))
    }

    // ── Phase 2: encode text ─────────────────────────────────────────────
    /// Returns embeddings shaped (2, max_len, embed_dim) — uncond stacked
    /// before cond so the UNet's batch dim aligns with CFG split below.
    fn encode_text(&self, prompt: &str, negative: &str) -> Result<Tensor> {
        let (cond_ids, uncond_ids) = self.tokenize_prompts(prompt, negative)?;
        let cond = self.clip.forward(&cond_ids)?;
        let uncond = self.clip.forward(&uncond_ids)?;
        let embeds = Tensor::cat(&[&uncond, &cond], 0)?.to_dtype(self.dtype)?;
        Ok(embeds)
    }

    // ── Phase 3: initial noise ───────────────────────────────────────────
    fn sample_initial_latents(&self, width: usize, height: usize, seed: u64) -> Result<Tensor> {
        // Seeded RNG → reproducible images. We set on the device so the noise
        // sample is identical across CPU/GPU runs (assuming Candle's RNG is
        // device-portable, which it is at 0.9).
        self.device.set_seed(seed)?;
        let latents = Tensor::randn(0f32, 1f32, (1, 4, height / 8, width / 8), &self.device)?
            .to_dtype(self.dtype)?;
        Ok(latents)
    }

    // ── Phase 5: VAE decode ──────────────────────────────────────────────
    fn decode_latents(&self, latents: &Tensor) -> Result<Tensor> {
        let latents = (latents / VAE_SCALE)?;
        let img = self.vae.decode(&latents)?;
        // VAE outputs in [-1, 1]; remap to [0, 1] then clamp.
        let img = ((img / 2.0)? + 0.5)?.clamp(0f32, 1f32)?;
        Ok(img)
    }
}

#[async_trait::async_trait]
impl ImageGenerator for StableDiffusionGenerator {
    fn name(&self) -> &str {
        "stable-diffusion-1.5 (candle)"
    }

    async fn generate(&self, opts: &GenOptions) -> Result<RgbImage> {
        // The Candle ops are CPU-blocking (Metal/CUDA syncs happen in-call),
        // so we hop to a blocking thread to keep tokio happy.
        let prompt = opts.prompt.clone();
        let negative = opts.negative_prompt.clone().unwrap_or_default();
        let width = opts.resolution.width as usize;
        let height = opts.resolution.height as usize;
        let steps = opts.steps;
        let cfg = opts.guidance_scale;
        let seed = opts.seed.unwrap_or_else(rand::random);

        ui::status(&format!(
            "generating: {res}, {steps} steps, cfg={cfg}, seed={seed}",
            res = opts.resolution
        ));

        // SAFETY: tokio::task::block_in_place would work too, but spawn_blocking
        // is friendlier to the runtime. We move clones of cheap handles in;
        // the heavy model state stays where it is via &self capture isn't
        // possible across thread boundary, so we do the work synchronously
        // here under block_in_place instead.
        let img = tokio::task::block_in_place(|| -> Result<RgbImage> {
            self.run_sync(&prompt, &negative, width, height, steps, cfg, seed)
        })?;

        Ok(img)
    }
}

impl StableDiffusionGenerator {
    fn run_sync(
        &self,
        prompt: &str,
        negative: &str,
        width: usize,
        height: usize,
        steps: usize,
        cfg: f64,
        seed: u64,
    ) -> Result<RgbImage> {
        let text_embeds = self.encode_text(prompt, negative)?;
        let mut scheduler = self.config.build_scheduler(steps)?;
        let mut latents = self.sample_initial_latents(width, height, seed)?;
        latents = (latents * scheduler.init_noise_sigma())?;

        // Snapshot timesteps into an owned Vec so we can pass &mut scheduler
        // into scheduler.step() inside the loop without a borrow conflict.
        let timesteps: Vec<usize> = scheduler.timesteps().to_vec();

        // ── Phase 4: denoising loop with CFG ────────────────────────────
        for (step_idx, &timestep) in timesteps.iter().enumerate() {
            // Duplicate latents for [uncond, cond] in the batch dim.
            let latent_in = Tensor::cat(&[&latents, &latents], 0)?;
            let latent_in = scheduler.scale_model_input(latent_in, timestep)?;

            let noise_pred = self
                .unet
                .forward(&latent_in, timestep as f64, &text_embeds)?;
            // Split batch back into the two predictions and apply CFG.
            let noise_uncond = noise_pred.i(0)?.unsqueeze(0)?;
            let noise_cond = noise_pred.i(1)?.unsqueeze(0)?;
            let noise_pred = (&noise_uncond + ((noise_cond - &noise_uncond)? * cfg)?)?;

            latents = scheduler.step(&noise_pred, timestep, &latents)?;

            if step_idx % 5 == 0 || step_idx + 1 == steps {
                ui::status(&format!("  step {}/{}", step_idx + 1, steps));
            }
        }

        let img_t = self.decode_latents(&latents)?;
        tensor_to_image(&img_t)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

async fn fetch(repo: &hf_hub::api::tokio::ApiRepo, file: &str) -> Result<PathBuf> {
    repo.get(file)
        .await
        .with_context(|| format!("failed to fetch {file}"))
}

fn select_device() -> Result<Device> {
    #[cfg(feature = "metal")]
    {
        return Ok(Device::new_metal(0)?);
    }
    #[cfg(all(feature = "cuda", not(feature = "metal")))]
    {
        return Ok(Device::new_cuda(0)?);
    }
    #[cfg(not(any(feature = "metal", feature = "cuda")))]
    {
        Ok(Device::Cpu)
    }
}

/// Convert a (1, 3, H, W) tensor in [0, 1] into a `RgbImage`.
fn tensor_to_image(t: &Tensor) -> Result<RgbImage> {
    let t = t.squeeze(0)?.to_dtype(DType::F32)?;
    let (channels, height, width) = t.dims3()?;
    anyhow::ensure!(channels == 3, "expected 3-channel image, got {channels}");

    // Move to CPU + permute to (H, W, C) for byte-packing.
    let t = t.permute((1, 2, 0))?.to_device(&Device::Cpu)?;
    let data: Vec<f32> = t.flatten_all()?.to_vec1()?;
    let bytes: Vec<u8> = data
        .into_iter()
        .map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8)
        .collect();

    RgbImage::from_raw(width as u32, height as u32, bytes)
        .context("RgbImage::from_raw failed — wrong buffer size")
}
