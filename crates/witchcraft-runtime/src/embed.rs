//! Deterministic embedding + similarity math for the compiled runtime.
//!
//! This is intentionally a byte-for-byte copy of the interpreter's
//! `witchcraft::interp::embed_vector`/`cosine` and the `nearest` ranking, so the
//! compiled `embed`/`similarity`/`nearest` agree with `witch run` on the Mock
//! engine. The codegen equivalence tests guard against drift (the same precedent
//! as the duplicated decoder).

/// A fixed-dimension, deterministic embedding derived from the text and its
/// space. Same text + space always yields the same vector.
pub fn embed_vector(text: &str, space: &str) -> Vec<f64> {
    const DIMS: usize = 16;
    let mut v = vec![0.0f64; DIMS];
    for token in text.split_whitespace() {
        for (d, slot) in v.iter_mut().enumerate() {
            let h = fnv1a(&format!("{space}\u{0}{token}\u{0}{d}"));
            *slot += ((h % 2000) as f64) / 1000.0 - 1.0;
        }
    }
    if text.split_whitespace().next().is_none() {
        for (d, slot) in v.iter_mut().enumerate() {
            let h = fnv1a(&format!("{space}\u{0}<empty>\u{0}{d}"));
            *slot = ((h % 2000) as f64) / 1000.0 - 1.0;
        }
    }
    v
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Cosine similarity of two equal-length vectors (0 if either is the zero
/// vector). Cross-space comparison is a compile error on both paths, so this is
/// only ever called with same-space vectors.
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Rank candidate indices against a query vector: descending by cosine, ties
/// broken by original index (deterministic), keeping the top `k`. Mirrors the
/// interpreter's `nearest`.
pub fn rank_top_k(query: &[f64], candidates: &[Vec<f64>], k: usize) -> Vec<usize> {
    let mut scored: Vec<(usize, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i, cosine(query, c)))
        .collect();
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    scored.into_iter().take(k).map(|(i, _)| i).collect()
}
