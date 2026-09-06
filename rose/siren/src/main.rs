mod candle_sd;
mod generator;
mod session;
mod ui;
mod viewer;

use std::path::PathBuf;

use clap::Parser;

use crate::candle_sd::StableDiffusionGenerator;
use crate::session::Session;

#[derive(Parser)]
#[command(
    name = "siren",
    about = "Interactive text-to-image studio. Prompt → preview → reprompt → save."
)]
struct Cli {
    /// Initial prompt. Can be multi-word without quotes.
    #[arg(required = true)]
    prompt: Vec<String>,

    /// Directory where final saved images land.
    #[arg(long, short, default_value = "./siren-output")]
    output_dir: PathBuf,

    /// Number of denoising steps for the *preview* renders. Saving uses the
    /// same value. Lower = faster iteration; 20-30 is the sweet spot for SD 1.5.
    #[arg(long, default_value_t = 25)]
    steps: usize,

    /// Classifier-free guidance scale. Higher = more prompt-faithful but
    /// sometimes oversaturated. SD 1.5's sweet spot is 7-9.
    #[arg(long, default_value_t = 7.5)]
    guidance: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    ui::banner();

    let prompt = cli.prompt.join(" ");
    ui::status(&format!("prompt: \"{prompt}\""));

    let generator = StableDiffusionGenerator::load().await?;

    let mut session = Session::new(generator, cli.output_dir, prompt);
    // Wire CLI sampling knobs into the session's starting options.
    session.set_steps(cli.steps);
    session.set_guidance(cli.guidance);

    session.run().await
}
