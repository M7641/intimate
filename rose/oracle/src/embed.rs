//! Embeddings + the learned-relevance scorer (the *fast* feedback loop).
//!
//! ## Why a lexical embedder for the pilot
//!
//! The architecture below — embed everything, score new items against the
//! centroid of what you liked minus what you rejected — is identical whether the
//! vectors are lexical (bag-of-hashed-tokens) or dense semantic vectors from an
//! embedding model. We ship a dependency-free lexical embedder so oracle runs
//! offline with zero API keys, and isolate it behind the [`Embedder`] trait so a
//! real model (e.g. a rig embedding provider) can be dropped in without touching
//! the curation logic. The lexical version captures topical/keyword overlap; the
//! upgrade buys semantic similarity. Same wiring either way.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
    fn dim(&self) -> usize;
}

/// Deterministic, offline lexical embedder: hash each token into a fixed-width
/// vector with sub-linear term weighting, then L2-normalise.
pub struct HashEmbedder {
    dim: usize,
}

impl HashEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

#[async_trait::async_trait]
impl Embedder for HashEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut v = vec![0.0f32; self.dim];
        for token in tokenize(text) {
            let mut h = DefaultHasher::new();
            token.hash(&mut h);
            let idx = (h.finish() as usize) % self.dim;
            // sign bit decorrelates collisions a little (the hashing-trick trick)
            let sign = if (h.finish() >> 1) & 1 == 0 { 1.0 } else { -1.0 };
            v[idx] += sign;
        }
        // sub-linear damping so a word repeated 50× doesn't dominate
        for x in v.iter_mut() {
            *x = x.signum() * (x.abs() + 1.0).ln();
        }
        l2_normalise(&mut v);
        Ok(v)
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

fn tokenize(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 2)
        .map(|t| t.to_lowercase())
}

fn l2_normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Cosine similarity of two equal-length vectors (already-normalised → dot product).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Mean vector of a set (the "centroid"), re-normalised.
pub fn centroid(vectors: &[Vec<f32>], dim: usize) -> Option<Vec<f32>> {
    if vectors.is_empty() {
        return None;
    }
    let mut c = vec![0.0f32; dim];
    for v in vectors {
        for (i, x) in v.iter().enumerate() {
            c[i] += x;
        }
    }
    for x in c.iter_mut() {
        *x /= vectors.len() as f32;
    }
    l2_normalise(&mut c);
    Some(c)
}

/// The learned relevance of a candidate: how much it looks like things you liked,
/// minus how much it looks like things you rejected. Range roughly [-1, 1].
///
/// This is the *fast* loop — it updates the instant you give feedback, with no
/// LLM call and no model training, because the centroids are recomputed from the
/// feedback table on every scoring pass.
pub fn relevance(candidate: &[f32], liked: Option<&Vec<f32>>, disliked: Option<&Vec<f32>>) -> f32 {
    let pos = liked.map(|c| cosine(candidate, c)).unwrap_or(0.0);
    let neg = disliked.map(|c| cosine(candidate, c)).unwrap_or(0.0);
    pos - neg
}

/// Serialise a vector as JSON for the `items.embedding` TEXT column.
pub fn encode(v: &[f32]) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())
}

/// Parse a vector back from the `items.embedding` column.
pub fn decode(s: &str) -> Vec<f32> {
    serde_json::from_str(s).unwrap_or_default()
}
