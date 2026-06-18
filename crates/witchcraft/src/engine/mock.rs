//! The deterministic, grammar-respecting Mock engine — demoted from the only
//! decoder to a selectable *test* engine, but retained as the offline default
//! and the deterministic litmus oracle. It honours the grammar token-by-token
//! (illegal outputs are unreachable) and makes no network calls.
//!
//! Its value generation delegates to [`crate::decoder::MockDecoder`] unchanged,
//! so the compiled/interpreted equivalence (the runtime mirrors that exact
//! algorithm) is preserved. The trace is computed from the grammar so the
//! falsification test can prove masking occurred during generation.

use crate::decoder::{Decoder, MockDecoder};
use crate::engine::{
    permitted_tokens, DecodeStep, DecodeTrace, Engine, EngineDescription, InferRequest,
    InferResult, LatencyClass, Locality, Modality,
};
use crate::grammar::Grammar;
use crate::value::{Provenance, Value};

pub struct MockEngine {
    decoder: MockDecoder,
    model_id: String,
    model_version: String,
}

impl MockEngine {
    pub fn new(seed: u64, model_id: impl Into<String>) -> Self {
        MockEngine {
            decoder: MockDecoder::new(seed),
            model_id: model_id.into(),
            model_version: "mock".to_string(),
        }
    }

    fn provenance(&self, intent_id: &str, seed: u64) -> Provenance {
        // With no manifest binding, the intent *is* the resolved model id (the
        // Mock stands in for whatever the deployment would bind).
        let model = if self.model_id.is_empty() {
            intent_id.to_string()
        } else {
            self.model_id.clone()
        };
        Provenance {
            oracle: intent_id.to_string(),
            model,
            model_version_or_sha: self.model_version.clone(),
            backend_id: "mock".to_string(),
            seed,
            sampling: "deterministic".to_string(),
        }
    }
}

impl Engine for MockEngine {
    fn describe(&self) -> EngineDescription {
        EngineDescription {
            backend_id: "mock".to_string(),
            tiers: vec!["test".to_string()],
            modalities: vec![Modality::Text],
            locality: Locality::Local,
            latency_class: LatencyClass::Interactive,
            grammar_constrained: true,
            // The Mock masks the grammar by construction — it is the litmus oracle.
            litmus_safe: Some(true),
            non_litmus_safe_reason: None,
        }
    }

    fn infer(&mut self, req: &InferRequest) -> InferResult {
        let decoded = self.decoder.decode(req.grammar);
        InferResult {
            value: decoded.value,
            confidence: decoded.confidence,
            provenance: self.provenance(req.intent_id, req.seed),
        }
    }

    fn infer_traced(&mut self, req: &InferRequest) -> (InferResult, Option<DecodeTrace>) {
        let result = self.infer(req);
        let trace = build_trace(req.grammar, &result.value);
        (result, Some(trace))
    }

    fn embed(&mut self, _intent_id: &str, input: &str, space: &str) -> Value {
        embed_hash(input, space)
    }
}

/// Build a per-step trace: the permitted next-token set at each decode step, plus
/// the token the generated value chose where derivable. The permitted sets are
/// what the falsification test contrasts (real vs weakened) to prove masking.
fn build_trace(grammar: &Grammar, value: &Value) -> DecodeTrace {
    let permitted = permitted_tokens(grammar);
    let chosen = chosen_tokens(value);
    permitted
        .into_iter()
        .enumerate()
        .map(|(i, toks)| DecodeStep {
            chosen: chosen
                .get(i)
                .cloned()
                .unwrap_or_else(|| toks.first().cloned().unwrap_or_default()),
            permitted: toks,
        })
        .collect()
}

/// Best-effort projection of a generated value back onto the canonical tokens, in
/// the same depth-first order `permitted_tokens` walks the grammar.
fn chosen_tokens(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk_chosen(value, &mut out);
    out
}

fn walk_chosen(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Spark(n) => out.push((*n as i64).to_string()),
        Value::Bool(b) => out.push(b.to_string()),
        Value::Glyph(s) => out.push(s.chars().next().map(|c| c.to_string()).unwrap_or_default()),
        Value::Record { fields, .. } => {
            for (_, v) in fields {
                walk_chosen(v, out);
            }
        }
        Value::Variant { name, .. } => out.push(name.clone()),
        _ => {}
    }
}

/// Deterministic embedding tagged with its space, reusing the interpreter's
/// existing `embed_vector` so the Mock engine's `embed` matches typed-embedding
/// behaviour exactly (similarity/nearest stay reproducible).
pub fn embed_hash(input: &str, space: &str) -> Value {
    Value::Embedding {
        space: space.to_string(),
        vector: crate::interp::embed_vector(input, space),
        provenance: None,
    }
}
