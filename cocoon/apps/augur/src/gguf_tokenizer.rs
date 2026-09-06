//! Rebuild a tokenizer from the GGUF's own embedded vocabulary.
//!
//! Some models (e.g. Sweep Next-Edit) ship only a GGUF, with a custom
//! vocabulary baked into it and no `tokenizer.json`. An off-the-shelf tokenizer
//! then mismatches the model's embedding table and every token is wrong.
//! llama.cpp sidesteps this by reading the embedded tokenizer directly; candle
//! needs a `tokenizers::Tokenizer`, so we reconstruct one from the GGUF's
//! `tokenizer.ggml.*` metadata — a byte-level BPE, the format these
//! Qwen-derived GGUFs use.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use candle_core::quantized::gguf_file::Value;
use serde_json::{Map, Value as Json, json};
use tokenizers::Tokenizer;

/// Returns true if the GGUF carries a tokenizer we know how to rebuild.
pub fn is_present(md: &HashMap<String, Value>) -> bool {
    md.get("tokenizer.ggml.model")
        .and_then(|v| v.to_string().ok())
        .map(|s| s == "gpt2")
        .unwrap_or(false)
        && md.contains_key("tokenizer.ggml.tokens")
}

pub fn from_gguf_metadata(md: &HashMap<String, Value>) -> Result<Tokenizer> {
    if !is_present(md) {
        bail!("GGUF has no rebuildable gpt2 tokenizer");
    }

    let tokens = str_array(md, "tokenizer.ggml.tokens")?;
    let merges = str_array(md, "tokenizer.ggml.merges")?;
    let token_type = int_array(md, "tokenizer.ggml.token_type").unwrap_or_default();

    // token -> id
    let mut vocab = Map::with_capacity(tokens.len());
    for (i, t) in tokens.iter().enumerate() {
        vocab.insert(t.clone(), json!(i as u32));
    }

    // Control tokens (type 3) become special added tokens, so they are matched
    // as single units in the input text rather than split into characters.
    let added: Vec<Json> = tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| token_type.get(*i).copied() == Some(3))
        .map(|(i, t)| {
            json!({
                "id": i as u32, "content": t, "single_word": false,
                "lstrip": false, "rstrip": false, "normalized": false, "special": true
            })
        })
        .collect();

    // The same byte-level BPE layout HF emits for these models, assembled here
    // from GGUF data instead of read from a file.
    let spec = json!({
        "version": "1.0", "truncation": null, "padding": null,
        "added_tokens": added,
        "normalizer": null,
        "pre_tokenizer": {"type": "ByteLevel", "add_prefix_space": false, "trim_offsets": true, "use_regex": true},
        "post_processor": {"type": "ByteLevel", "add_prefix_space": true, "trim_offsets": false, "use_regex": true},
        "decoder": {"type": "ByteLevel", "add_prefix_space": true, "trim_offsets": true, "use_regex": true},
        "model": {
            "type": "BPE", "dropout": null, "unk_token": null,
            "continuing_subword_prefix": null, "end_of_word_suffix": null,
            "fuse_unk": false, "byte_fallback": false,
            "vocab": Json::Object(vocab), "merges": merges
        },
    });

    let bytes = serde_json::to_vec(&spec).context("serialize tokenizer spec")?;
    Tokenizer::from_bytes(&bytes).map_err(|e| anyhow::anyhow!("build tokenizer from gguf: {e}"))
}

fn str_array(md: &HashMap<String, Value>, key: &str) -> Result<Vec<String>> {
    let arr = md
        .get(key)
        .with_context(|| format!("missing {key}"))?
        .to_vec()
        .map_err(|e| anyhow::anyhow!("{key} not an array: {e}"))?;
    Ok(arr
        .iter()
        .filter_map(|v| v.to_string().ok().cloned())
        .collect())
}

fn int_array(md: &HashMap<String, Value>, key: &str) -> Option<Vec<i64>> {
    let arr = md.get(key)?.to_vec().ok()?;
    Some(
        arr.iter()
            .map(|v| {
                v.to_i32()
                    .map(|x| x as i64)
                    .or_else(|_| v.to_u32().map(|x| x as i64))
                    .unwrap_or(0)
            })
            .collect(),
    )
}
