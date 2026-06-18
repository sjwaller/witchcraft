//! The local llama.cpp engine (change `add-inference-runtime`) — the Break 1
//! target. It is litmus-safe by construction: the output type is compiled to GBNF
//! (see [`crate::engine::grammar_to_gbnf`]) and llama.cpp's grammar sampler masks
//! the logits **token-by-token**, so illegal outputs are unreachable. This is the
//! safest way to prove the litmus against a real tokenizer — we do not
//! hand-author the masking mechanism.
//!
//! Built only with `--features llama`, which links `libllama` (C/C++). The link
//! is confined to model EXECUTION; the language/toolchain stays self-contained.
//! The GGUF model artifact sits beside the binary (named only in the manifest).
//!
//! NOTE: this module is exercised in CI with a real GGUF model behind the
//! `llama` feature; the default offline build never compiles or links it.

use std::num::NonZeroU32;
use std::sync::OnceLock;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::data::LlamaTokenData;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;

use crate::engine::{
    fallback_value, grammar_to_gbnf, json_to_value, DecodeStep, DecodeTrace, Engine,
    EngineDescription, InferRequest, InferResult, LatencyClass, Locality, Modality,
};
use crate::grammar::Grammar;
use crate::manifest::{EngineSpec, NeedBinding};
use crate::value::{Provenance, Value};

/// The global llama backend (init exactly once per process).
fn backend() -> &'static LlamaBackend {
    static B: OnceLock<LlamaBackend> = OnceLock::new();
    B.get_or_init(|| LlamaBackend::init().expect("llama.cpp backend initialises"))
}

pub struct LlamaEngine {
    model: LlamaModel,
    model_id: String,
    sha: String,
    n_ctx: u32,
}

impl LlamaEngine {
    /// Load the GGUF model named by the manifest binding.
    pub fn from_spec(binding: &NeedBinding, spec: &EngineSpec) -> Result<Self, String> {
        let path = spec
            .params
            .get("gguf")
            .ok_or_else(|| "llama engine spec missing `gguf = \"...path...\"`".to_string())?;
        let model = LlamaModel::load_from_file(backend(), path, &LlamaModelParams::default())
            .map_err(|e| format!("failed to load GGUF model `{path}`: {e}"))?;
        Ok(LlamaEngine {
            model,
            model_id: if binding.model.is_empty() {
                path.clone()
            } else {
                binding.model.clone()
            },
            sha: binding
                .sha256
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            n_ctx: 2048,
        })
    }

    fn provenance(&self, intent_id: &str, seed: u64) -> Provenance {
        Provenance {
            oracle: intent_id.to_string(),
            model: self.model_id.clone(),
            model_version_or_sha: self.sha.clone(),
            backend_id: "llama".to_string(),
            seed,
            sampling: "grammar+dist".to_string(),
        }
    }

    /// Generate a grammar-constrained string for the request, returning the raw
    /// text the GBNF admitted (JSON-ish for aggregates, a bare token for scalars).
    fn generate(&self, req: &InferRequest) -> Result<String, String> {
        let ctx_params = LlamaContextParams::default().with_n_ctx(NonZeroU32::new(self.n_ctx));
        let mut ctx = self
            .model
            .new_context(backend(), ctx_params)
            .map_err(|e| format!("context: {e}"))?;

        let tokens = self
            .model
            .str_to_token(req.input, AddBos::Always)
            .map_err(|e| format!("tokenize: {e}"))?;

        let mut batch = LlamaBatch::new(512, 1);
        let last = tokens.len().saturating_sub(1);
        for (i, t) in tokens.iter().enumerate() {
            batch
                .add(*t, i as i32, &[0], i == last)
                .map_err(|e| format!("batch: {e}"))?;
        }
        ctx.decode(&mut batch).map_err(|e| format!("decode: {e}"))?;

        let gbnf = grammar_to_gbnf(req.grammar);
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::grammar(&self.model, &gbnf, "root"),
            LlamaSampler::dist(req.seed),
        ]);

        let mut out = String::new();
        let mut pos = tokens.len() as i32;
        for _ in 0..512 {
            let idx = batch.n_tokens() - 1;
            let token = sampler.sample(&ctx, idx);
            sampler.accept(token);
            if self.model.is_eog_token(token) {
                break;
            }
            if let Ok(piece) = self.model.token_to_str(token, Special::Plaintext) {
                out.push_str(&piece);
            }
            batch.clear();
            batch
                .add(token, pos, &[0], true)
                .map_err(|e| format!("batch: {e}"))?;
            ctx.decode(&mut batch).map_err(|e| format!("decode: {e}"))?;
            pos += 1;
        }
        Ok(out)
    }

    /// Probe the live grammar mask at the first decode step: for each candidate
    /// token the *weakened* grammar would permit, ask whether the *real* grammar's
    /// sampler forbids it (drives its logit to -inf). This produces the permitted
    /// set the falsification test needs to prove masking occurred during
    /// generation (Verification B), using the engine's real sampler — not a
    /// re-derivation of the grammar.
    fn permitted_first_step(&self, grammar: &Grammar) -> Vec<String> {
        let gbnf = grammar_to_gbnf(grammar);
        let mut grammar_sampler = LlamaSampler::grammar(&self.model, &gbnf, "root");
        let candidates = crate::engine::permitted_tokens(&Grammar::Text { max_len: 1 })
            .into_iter()
            .next()
            .unwrap_or_default();
        let mut permitted = Vec::new();
        for cand in candidates {
            if let Ok(toks) = self.model.str_to_token(&cand, AddBos::Never) {
                if let Some(tok) = toks.first() {
                    let data = LlamaTokenData::new(*tok, 1.0, 0.0);
                    let mut arr = LlamaTokenDataArray::new(vec![data], false);
                    grammar_sampler.apply(&mut arr);
                    let logit = arr
                        .data
                        .first()
                        .map(|d| d.logit())
                        .unwrap_or(f32::NEG_INFINITY);
                    let forbidden = logit.is_infinite() && logit.is_sign_negative();
                    if !forbidden {
                        permitted.push(cand);
                    }
                }
            }
        }
        permitted
    }
}

impl Engine for LlamaEngine {
    fn describe(&self) -> EngineDescription {
        EngineDescription {
            backend_id: "llama".to_string(),
            tiers: vec!["standard".to_string()],
            modalities: vec![Modality::Text],
            locality: Locality::Local,
            latency_class: LatencyClass::Interactive,
            grammar_constrained: true,
            // GBNF masks token-by-token — litmus-safe by construction.
            litmus_safe: Some(true),
            non_litmus_safe_reason: None,
        }
    }

    fn infer(&mut self, req: &InferRequest) -> InferResult {
        let text = self.generate(req).unwrap_or_default();
        let value = json_to_value(&text, req.grammar)
            .or_else(|| scalar_from_text(&text, req.grammar))
            .unwrap_or_else(|| fallback_value(req.grammar));
        InferResult {
            value,
            confidence: 1.0,
            provenance: self.provenance(req.intent_id, req.seed),
        }
    }

    fn infer_traced(&mut self, req: &InferRequest) -> (InferResult, Option<DecodeTrace>) {
        let permitted = self.permitted_first_step(req.grammar);
        let result = self.infer(req);
        let chosen = permitted.first().cloned().unwrap_or_default();
        let trace = vec![DecodeStep { permitted, chosen }];
        (result, Some(trace))
    }
}

/// Parse a bare scalar generation (number/bool/text) that is not JSON.
fn scalar_from_text(text: &str, grammar: &Grammar) -> Option<Value> {
    let t = text.trim();
    match grammar {
        Grammar::Number { .. } => t.parse::<i64>().ok().map(|n| Value::Spark(n as f64)),
        Grammar::Bool => t.parse::<bool>().ok().map(Value::Bool),
        Grammar::Text { .. } => Some(Value::Glyph(t.to_string())),
        _ => None,
    }
}
