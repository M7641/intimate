use crate::llm::LlmClient;
use crate::ui;

const PREAMBLE: &str = "You are a helpful assistant. Answer clearly and concisely.";
const MODEL: &str = "opus";

pub async fn run(llm: &dyn LlmClient, question: &str) -> anyhow::Result<()> {
    let response = llm
        .prompt_with_model(PREAMBLE, question, Some(MODEL))
        .await?;
    ui::response(&response);
    Ok(())
}
