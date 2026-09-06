# Introduction to RAG (Retrieval-Augmented Generation) in Rust

## What is RAG?

Retrieval-Augmented Generation is a technique that enhances the output of large language models (LLMs) by grounding them in external, factual data at inference time. Rather than relying solely on what a model learned during training — knowledge that is frozen at a point in time and may be incomplete or incorrect — RAG systems first *retrieve* relevant documents from a knowledge base, then *augment* the prompt with that context before the model *generates* a response.

The architecture follows a straightforward pipeline:

1. **Ingest** — Documents are split into chunks, each chunk is converted into a vector embedding, and these embeddings are stored in a vector database or index.
2. **Retrieve** — When a user submits a query, it is also embedded into a vector, and a similarity search finds the most relevant chunks.
3. **Generate** — The retrieved chunks are injected into the LLM's prompt as context, and the model produces an answer grounded in that material.

This approach was popularised by a 2020 paper from Facebook AI Research (Lewis et al.) and has since become one of the most practical patterns for building knowledge-aware AI applications.

## Why RAG Adds Value

### Grounding and accuracy

An LLM on its own can hallucinate — it generates plausible-sounding text that may be factually wrong. By supplying relevant source documents in the prompt, RAG constrains the model's output to information that actually exists in your corpus. This dramatically reduces hallucination for domain-specific questions.

### Fresh knowledge without retraining

Fine-tuning a model is expensive and slow. RAG sidesteps this entirely: when your data changes, you update the vector index. The model sees the latest information at query time without any retraining step.

### Transparency and citations

Because you know which documents were retrieved, you can surface citations alongside the generated answer. Users can verify claims against the original source, which builds trust and makes the system auditable.

### Cost efficiency

Rather than training a massive model to memorise your entire corpus, you keep a smaller (and cheaper) model and pair it with a retrieval layer. This is often an order of magnitude less expensive than fine-tuning, and it scales naturally as your document collection grows.

### Access control

RAG makes it straightforward to enforce per-user permissions. You filter the retrieval step so that users only see documents they are authorised to access — something that is essentially impossible to achieve with fine-tuning alone.

---

## But How Does the LLM Know to Trigger a RAG Search?

Understanding the RAG pipeline itself is only half the picture. The other half is the bridge between a user asking a question and the system deciding to search a knowledge base. This is where **tool use** and the **orchestration loop** come in.

### Not all LLMs can use tools

Tool use is not an innate capability of language models. A vanilla model trained purely on text completion has no concept of tools. If you put a tool description in its prompt and ask a question, it might generate something that *looks* like a tool call by pattern-matching the format you showed it, but it won't do so reliably.

There is a spectrum of capability:

**No training** — You can hack tool use into any model by prompting it to output structured text and parsing the output yourself, but the model doesn't understand what it's doing. It's just completing a pattern. This is how people first experimented with tool use on earlier open models.

**Fine-tuned tool use** — Models like Claude, GPT-4, and Gemini have been explicitly fine-tuned on datasets of conversations that include tool calls. During this training, the model sees thousands of examples where the correct "next token" is a structured tool invocation rather than a natural language word. It learns patterns like: "when the user asks a factual question about internal data, and a search tool is available, the highest-reward action is to call that tool rather than attempt an answer from memory."

**Specialised agent models** — Some models are fine-tuned specifically for agentic behaviour, meaning they're trained not just on single tool calls but on multi-step tool-use trajectories. They've seen training examples of entire orchestration loops: call tool A, interpret the result, decide to call tool B, synthesise a final answer.

### How the model "sees" tools at runtime

The model has no persistent memory of tools between conversations. Every single time, the tool definitions are injected into the prompt — usually in a structured format the model was trained to recognise. For Claude, the API has a dedicated `tools` parameter. For OpenAI, it's the `functions` or `tools` field. Under the hood, these get serialised into the prompt in a specific format the model learned during fine-tuning.

If you don't pass any tools, the model won't generate tool calls — it has nothing to call. And if you trained a model on tool calls formatted as JSON but then gave it tool descriptions in XML at inference time, it would struggle. The model learned to associate a particular prompt structure with the behaviour of producing tool calls.

### It's still prediction — shaped by training

The model doesn't "know" there's a tool in some deep sense. It sees the tool descriptions in its context, and its training has taught it that certain patterns of user queries plus available tool descriptions should produce tool-call outputs rather than text outputs. It's still next-token prediction, but prediction that has been deliberately shaped through fine-tuning so that it reliably produces the right kind of output at the right time.

---

## The Orchestration Loop: Bridging Questions to Tool Calls

The bridge between a user's question and a RAG lookup is an **orchestration loop** that sits around the model. The model itself never executes anything — it only produces text (or structured text) and a surrounding program acts on it.

### Step by step

**1. The system prompt defines the tools.** Before the model ever sees the user's question, it receives a description of every tool available to it:

```json
{
  "name": "search_knowledge_base",
  "description": "Search internal documents for information relevant to the user's query",
  "parameters": {
    "query": { "type": "string" }
  }
}
```

**2. The model decides whether to call a tool.** When the question arrives, the model's next-token prediction can produce a special structured output that says "I want to call a tool." This isn't a separate system making the decision; it's the same autoregressive generation. For a question like "What is our refund policy?", the model might generate:

```json
{
  "tool_call": "search_knowledge_base",
  "arguments": { "query": "refund policy" }
}
```

**3. The orchestrator intercepts and executes.** A loop running outside the model catches this. In Rust:

```rust
loop {
    let response = call_llm(&messages).await?;

    match response {
        LlmOutput::ToolCall(call) => {
            // The model wants to use a tool — execute it
            let result = execute_tool(&call).await?;

            // Feed the result back as a new message
            messages.push(Message::tool_result(call.id, result));

            // Loop again — let the model see the result
            // and decide what to do next
        }
        LlmOutput::Text(answer) => {
            // The model is done — return the final answer
            return Ok(answer);
        }
    }
}
```

**4. The RAG pipeline lives inside that tool.** When the orchestrator sees `search_knowledge_base`, it runs the retrieval pipeline — embed the query, search the vector index, return the top-k chunks. Those chunks come back to the model as a new message in the conversation.

**5. The model generates the final answer.** On the next pass through the loop, the model sees the retrieved documents in its context and produces a natural language response grounded in them. No tool call this time — just text. The loop exits.

The intelligence is in the prediction; the action is in the loop.

---

## Implementing RAG in Rust

Rust is a compelling choice for RAG systems. Its performance characteristics — zero-cost abstractions, no garbage collector, and predictable latency — make it well-suited for the embedding, indexing, and retrieval stages where throughput matters.

### Key crates

| Crate | Purpose |
|---|---|
| `reqwest` | HTTP client for calling embedding and LLM APIs |
| `serde` / `serde_json` | Serialisation of documents, embeddings, and API payloads |
| `hnsw_rs` or `hora` | Approximate nearest-neighbour (ANN) search for the vector index |
| `tokenizers` | HuggingFace tokenizer bindings for chunk-size control |
| `text-splitter` | Document chunking with overlap |
| `tokio` | Async runtime for concurrent API calls |
| `qdrant-client` | Client for the Qdrant vector database (if using an external store) |

### Step 1 — Chunking documents

Before you can embed anything, you need to break long documents into chunks that fit within the embedding model's context window (typically 512 tokens):

```rust
use text_splitter::TextSplitter;

fn chunk_document(text: &str, max_tokens: usize) -> Vec<String> {
    let splitter = TextSplitter::new(max_tokens);
    splitter
        .chunks(text)
        .map(|c| c.to_string())
        .collect()
}
```

You generally want some overlap between chunks (50–100 tokens) so that context is not lost at boundaries.

### Step 2 — Generating embeddings

You need a vector representation of each chunk. Most RAG systems use an embedding API such as OpenAI's `text-embedding-3-small` or a locally-hosted model:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

async fn get_embeddings(
    client: &Client,
    api_key: &str,
    texts: &[String],
) -> anyhow::Result<Vec<Vec<f32>>> {
    let body = EmbeddingRequest {
        model: "text-embedding-3-small".into(),
        input: texts.to_vec(),
    };

    let resp = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?
        .json::<EmbeddingResponse>()
        .await?;

    Ok(resp.data.into_iter().map(|d| d.embedding).collect())
}
```

If you prefer to run inference locally, the `candle` crate (from HuggingFace) lets you load ONNX or safetensors models directly in Rust.

### Step 3 — Building the vector index

For small-to-medium corpora, an in-process ANN index is perfectly adequate. The `hnsw_rs` crate provides a fast HNSW (Hierarchical Navigable Small World) implementation:

```rust
use hnsw_rs::prelude::*;

fn build_index(
    embeddings: &[(usize, Vec<f32>)],
) -> Hnsw<f32, DistCosine> {
    let max_nb_connection = 16;
    let ef_construction = 200;
    let nb_elements = embeddings.len();

    let hnsw = Hnsw::new(
        max_nb_connection,
        nb_elements,
        16,
        ef_construction,
        DistCosine,
    );

    for (id, emb) in embeddings {
        hnsw.insert(emb, *id);
    }

    hnsw
}
```

For production workloads needing persistence and horizontal scaling, Qdrant is popular in the Rust ecosystem because the server itself is written in Rust:

```rust
use qdrant_client::prelude::*;
use qdrant_client::qdrant::vectors_config::Config;
use qdrant_client::qdrant::{
    CreateCollection, Distance, VectorParams, VectorsConfig,
};

async fn setup_qdrant(client: &QdrantClient) -> anyhow::Result<()> {
    client
        .create_collection(&CreateCollection {
            collection_name: "documents".into(),
            vectors_config: Some(VectorsConfig {
                config: Some(Config::Params(VectorParams {
                    size: 1536,
                    distance: Distance::Cosine.into(),
                    ..Default::default()
                })),
            }),
            ..Default::default()
        })
        .await?;
    Ok(())
}
```

### Step 4 — Retrieval

When a query arrives, embed it using the same model, then search the index for the top-k nearest neighbours:

```rust
async fn retrieve(
    client: &Client,
    api_key: &str,
    index: &Hnsw<f32, DistCosine>,
    chunks: &[String],
    query: &str,
    top_k: usize,
) -> anyhow::Result<Vec<String>> {
    let query_emb = get_embeddings(client, api_key, &[query.to_string()])
        .await?
        .into_iter()
        .next()
        .unwrap();

    let ef_search = 64;
    let neighbours = index.search(&query_emb, top_k, ef_search);

    let results: Vec<String> = neighbours
        .iter()
        .map(|n| chunks[n.d_id].clone())
        .collect();

    Ok(results)
}
```

A good starting point is `top_k = 5`. You can tune this based on your chunk size and the LLM's context window.

### Step 5 — Augmented generation

Construct a prompt that includes the retrieved context and send it to the LLM:

```rust
#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

async fn generate_answer(
    client: &Client,
    api_key: &str,
    context_chunks: &[String],
    question: &str,
) -> anyhow::Result<String> {
    let context = context_chunks.join("\n\n---\n\n");

    let system_prompt = format!(
        "You are a helpful assistant. Answer the user's question based \
         only on the following context. If the context does not contain \
         enough information, say so.\n\n\
         Context:\n{context}"
    );

    let body = ChatRequest {
        model: "gpt-4o".into(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".into(),
                content: question.to_string(),
            },
        ],
        temperature: 0.2,
    };

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?
        .json::<ChatResponse>()
        .await?;

    Ok(resp.choices[0].message.content.clone())
}
```

Setting the temperature low (0.1–0.3) encourages the model to stick closely to the provided context.

### Putting it all together

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let client = reqwest::Client::new();

    // 1. Load and chunk your documents
    let raw_text = std::fs::read_to_string("knowledge_base.txt")?;
    let chunks = chunk_document(&raw_text, 400);

    // 2. Embed all chunks
    let embeddings = get_embeddings(&client, &api_key, &chunks).await?;

    // 3. Build the vector index
    let indexed: Vec<(usize, Vec<f32>)> = embeddings
        .into_iter()
        .enumerate()
        .collect();
    let index = build_index(&indexed);

    // 4. Handle a query
    let question = "What are the benefits of RAG?";
    let context = retrieve(
        &client, &api_key, &index, &chunks, question, 5
    ).await?;

    // 5. Generate the answer
    let answer = generate_answer(
        &client, &api_key, &context, question
    ).await?;

    println!("{answer}");
    Ok(())
}
```

### Cargo.toml dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
hnsw_rs = "0.3"
text-splitter = "0.16"
# Optional: for external vector DB
# qdrant-client = "1"
```

---

## Where to Go from Here

This article covered the foundational pattern. Production RAG systems typically layer on additional techniques:

- **Hybrid search** — combining vector similarity with BM25 keyword search for better recall. The `tantivy` crate is a full-text search engine written in Rust that pairs well with a vector index.
- **Re-ranking** — after retrieving an initial set of candidates, a cross-encoder model scores each (query, chunk) pair more accurately. This can be done via an API call or locally with `candle`.
- **Metadata filtering** — attaching metadata (date, author, category) to chunks and filtering at retrieval time so the model only sees relevant subsets.
- **Query transformation** — rewriting or expanding the user's query before embedding it. Techniques include HyDE (Hypothetical Document Embeddings) and multi-query retrieval.
- **Streaming responses** — using `reqwest`'s streaming support with `tokio` to pipe tokens to the client as they arrive, reducing perceived latency.
- **Evaluation** — measuring retrieval quality (recall@k, MRR) and generation quality (faithfulness, relevance) to guide tuning decisions.

Rust gives you the performance headroom to run these additional stages without blowing your latency budget, which is one of the strongest arguments for choosing it as the backbone of a RAG system.
