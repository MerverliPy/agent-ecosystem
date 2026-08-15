//! Local embeddings: a deterministic HashEmbedder (default, offline) and cosine
//! similarity. The real fastembed-rs embedder is gated behind the `fastembed`
//! feature so default builds/tests stay offline and dependency-light (DEC-0005).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const HASH_DIMS: usize = 64;

/// Anything that turns text into a normalized embedding vector.
/// `embed` takes `&mut self` because ONNX sessions (fastembed) require mutable access.
pub trait Embedder: Send + Sync {
    fn name(&self) -> &'static str;
    fn dims(&self) -> usize;
    fn embed(&mut self, text: &str) -> Vec<f32>;
}

/// Deterministic local embedder: token hashing into a fixed-dimension bag, L2-normalized.
/// Good enough for P0 retrieval; swap in `FastembedEmbedder` for real semantic recall.
pub struct HashEmbedder {
    dims: usize,
}

impl HashEmbedder {
    pub fn new(dims: usize) -> Self {
        Self { dims }
    }

    fn tokens(&self, text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .map(|t| t.to_lowercase())
            .filter(|t| !t.is_empty() && t.len() > 1)
            .collect()
    }
}

impl Default for HashEmbedder {
    fn default() -> Self {
        Self::new(HASH_DIMS)
    }
}

impl Embedder for HashEmbedder {
    fn name(&self) -> &'static str {
        "hash-v1"
    }

    fn dims(&self) -> usize {
        self.dims
    }

    fn embed(&mut self, text: &str) -> Vec<f32> {
        let mut vec = vec![0f32; self.dims];
        for token in self.tokens(text) {
            let mut h = DefaultHasher::new();
            token.hash(&mut h);
            let hash = h.finish();
            let idx = (hash % self.dims as u64) as usize;
            // sign from bit 32 so repeated tokens accumulate rather than cancel
            let sign = if hash & (1 << 32) == 0 { 1.0 } else { -1.0 };
            vec[idx] += sign;
        }
        l2_normalize(&mut vec);
        vec
    }
}

/// fastembed-rs wrapper (feature-gated). Downloads the model on first use and caches it.
#[cfg(feature = "fastembed")]
pub struct FastembedEmbedder {
    model: fastembed::TextEmbedding,
    dims: usize,
    name: &'static str,
}

#[cfg(feature = "fastembed")]
impl FastembedEmbedder {
    pub fn new() -> Result<Self, fastembed::Error> {
        let model_name = fastembed::EmbeddingModel::AllMiniLML12V2;
        let dims = fastembed::TextEmbedding::get_model_info(&model_name)?.dim;
        let model = fastembed::TextEmbedding::try_new(fastembed::TextInitOptions::new(model_name))?;
        Ok(Self {
            model,
            dims,
            name: "fastembed-all-minilm-l12-v2",
        })
    }
}

#[cfg(feature = "fastembed")]
impl Embedder for FastembedEmbedder {
    fn name(&self) -> &'static str {
        self.name
    }
    fn dims(&self) -> usize {
        self.dims
    }
    fn embed(&mut self, text: &str) -> Vec<f32> {
        let mut v = self
            .model
            .embed(vec![text], None)
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default();
        l2_normalize(&mut v);
        v
    }
}

pub fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Cosine similarity between two vectors (both assumed normalized).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embedder_is_deterministic_and_normalized() {
        let mut e = HashEmbedder::default();
        let a = e.embed("user prefers TypeScript");
        let b = e.embed("user prefers TypeScript");
        assert_eq!(a, b);
        assert_eq!(e.dims(), HASH_DIMS);
        let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn similar_texts_score_higher_than_dissimilar() {
        let mut e = HashEmbedder::default();
        let similar = cosine(&e.embed("user prefers TypeScript over JavaScript"), &e.embed("prefers TypeScript to JavaScript"));
        let dissimilar = cosine(&e.embed("user prefers TypeScript"), &e.embed("how to deploy the staging site"));
        assert!(similar > dissimilar, "similar={similar} dissimilar={dissimilar}");
    }

    #[test]
    fn cosine_handles_empty_and_mismatched() {
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
    }
}
