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
#[allow(deprecated)]
use llama_cpp_2::model::Special;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::data::LlamaTokenData;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;

use llama_cpp_2::{send_logs_to_tracing, LogOptions};

use crate::engine::{
    fallback_value, grammar_to_gbnf, json_to_value, DecodeStep, DecodeTrace, Engine,
    EngineDescription, InferRequest, InferResult, LatencyClass, Locality, Modality,
};
use crate::grammar::Grammar;
use crate::manifest::{EngineSpec, NeedBinding};
use crate::value::{Provenance, Value};

/// `true` when the user has opted into raw llama.cpp/ggml logging for debugging
/// via `WITCHCRAFT_LLAMA_VERBOSE=1` (or `true`). Default is fully silent.
fn llama_verbose() -> bool {
    std::env::var("WITCHCRAFT_LLAMA_VERBOSE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Silence llama.cpp's logging by default. llama.cpp emits through TWO separate
/// channels — the llama log stream and the ggml log stream — and the loudest,
/// load-time chatter (`llama_model_loader`, `create_tensor`, `load_tensors`,
/// `ggml_metal_*`, `print_info`) comes from ggml. `send_logs_to_tracing` sets
/// BOTH `llama_log_set` and `ggml_log_set`; with logs disabled the sink returns
/// immediately, so every channel is voided (a llama-only silencer misses ggml —
/// the prior bug). MUST run before the backend initialises and any model loads,
/// so load-time logging is captured too.
fn configure_logging() {
    if !llama_verbose() {
        send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
    }
}

/// The global llama backend (init exactly once per process). Logging is silenced
/// (unless `WITCHCRAFT_LLAMA_VERBOSE=1`) *before* `LlamaBackend::init`, so even
/// backend/Metal init and model-load logs are suppressed.
fn backend() -> &'static LlamaBackend {
    static B: OnceLock<LlamaBackend> = OnceLock::new();
    B.get_or_init(|| {
        configure_logging();
        LlamaBackend::init().expect("llama.cpp backend initialises")
    })
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
    /// text the GBNF admitted (JSON-ish for aggregates, a bare token for scalars)
    /// and a real confidence read from the sampler — the geometric mean of the
    /// probability the masked distribution assigned each chosen token.
    fn generate(&self, req: &InferRequest) -> Result<(String, f64), String> {
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
        let grammar_sampler = LlamaSampler::grammar(&self.model, &gbnf, "root")
            .map_err(|e| format!("grammar sampler: {e}"))?;
        let mut sampler =
            LlamaSampler::chain_simple([grammar_sampler, LlamaSampler::dist(req.seed as u32)]);

        // A SECOND grammar sampler, advanced in lockstep with the chain, lets us
        // read the masked distribution at each step to recover the chosen token's
        // probability — WITHOUT calling `accept` on the generation chain (that
        // would advance its grammar stacks twice and corrupt them, the documented
        // double-accept failure). The probe only ever `apply`s + `accept`s itself.
        let mut probe = LlamaSampler::grammar(&self.model, &gbnf, "root")
            .map_err(|e| format!("grammar sampler: {e}"))?;

        let mut out = String::new();
        let mut logp_sum = 0.0f64;
        let mut n_chosen = 0u32;
        let start = tokens.len() as i32;
        for pos in (start..).take(512) {
            let idx = batch.n_tokens() - 1;
            // Masked candidate distribution at this step (probe state == chain
            // grammar state, so the mask is identical to the chain's).
            let mut arr = ctx.token_data_array_ith(idx);
            arr.apply_sampler(&probe);

            // `llama_sampler_sample` applies the chain (grammar mask + dist) AND
            // accepts the chosen token internally — so we must NOT accept again on
            // `sampler`. The probe is advanced separately to stay in step.
            let token = sampler.sample(&ctx, idx);
            if self.model.is_eog_token(token) {
                break;
            }
            let p = chosen_probability(&arr, token);
            if p > 0.0 {
                logp_sum += (p as f64).ln();
                n_chosen += 1;
            }
            probe.accept(token);

            #[allow(deprecated)]
            if let Ok(piece) = self.model.token_to_str(token, Special::Plaintext) {
                out.push_str(&piece);
            }
            batch.clear();
            batch
                .add(token, pos, &[0], true)
                .map_err(|e| format!("batch: {e}"))?;
            ctx.decode(&mut batch).map_err(|e| format!("decode: {e}"))?;
        }
        // Geometric mean of per-token probabilities; a sequence confidence in
        // (0, 1]. With no tokens generated there is no signal — report 1.0 (the
        // grammar admitted the empty string), never fabricated certainty.
        let confidence = if n_chosen > 0 {
            (logp_sum / n_chosen as f64).exp()
        } else {
            1.0
        };
        Ok((out, confidence))
    }

    /// Probe the live grammar mask at the first decode step: for each candidate
    /// token the *weakened* grammar would permit, ask whether the *real* grammar's
    /// sampler forbids it (drives its logit to -inf). This produces the permitted
    /// set the falsification test needs to prove masking occurred during
    /// generation (Verification B), using the engine's real sampler — not a
    /// re-derivation of the grammar.
    fn permitted_first_step(&self, grammar: &Grammar) -> Vec<String> {
        let gbnf = grammar_to_gbnf(grammar);
        let grammar_sampler = match LlamaSampler::grammar(&self.model, &gbnf, "root") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let candidates = crate::engine::permitted_tokens(&Grammar::Text { max_len: 1 })
            .into_iter()
            .next()
            .unwrap_or_default();
        let mut permitted = Vec::new();
        for cand in candidates {
            if !self.token_forbidden(&grammar_sampler, &cand) {
                permitted.push(cand);
            }
        }
        permitted
    }

    /// Does `grammar_sampler`, in its current state, forbid the first token of
    /// `text` (drive its logit to -inf)? Shared by the first-step and the
    /// list-cardinality-boundary probes.
    fn token_forbidden(&self, grammar_sampler: &LlamaSampler, text: &str) -> bool {
        let toks = match self.model.str_to_token(text, AddBos::Never) {
            Ok(t) => t,
            Err(_) => return true,
        };
        let Some(tok) = toks.first() else {
            return true;
        };
        let data = LlamaTokenData::new(*tok, 1.0, 0.0);
        let mut arr = LlamaTokenDataArray::new(vec![data], false);
        grammar_sampler.apply(&mut arr);
        let logit = arr
            .data
            .first()
            .map(|d| d.logit())
            .unwrap_or(f32::NEG_INFINITY);
        logit.is_infinite() && logit.is_sign_negative()
    }

    /// The LIST-CARDINALITY masking probe (the on-thesis witness for this change).
    /// For a bounded `list of lo..hi of T`, build a JSON-array prefix of EXACTLY
    /// `hi` legal elements (no closing bracket), advance the real GBNF sampler
    /// across that prefix, then ask which of `{",", "]"}` the sampler still
    /// permits. A strict `0..hi` grammar MUST forbid `,` here (a `,` would open
    /// the (hi+1)-th element, exceeding the bound) while permitting `]`; a
    /// weakened `0..hi'` (hi' > hi) still permits `,`. Contrasting the two
    /// permitted sets is the proof that the COUNT BOUND masked generation
    /// token-by-token on real weights — the 5th exit is unreachable, not trimmed.
    ///
    /// Returns the permitted continuation tokens at the boundary, or an empty
    /// vec if the prefix could not be established on this tokenizer (the gated
    /// test then reports that honestly rather than asserting a false witness).
    fn list_boundary_permitted(&self, grammar: &Grammar) -> Vec<String> {
        let Grammar::List { elem, hi, .. } = grammar else {
            return Vec::new();
        };
        let gbnf = grammar_to_gbnf(grammar);
        let mut sampler = match LlamaSampler::grammar(&self.model, &gbnf, "root") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        // A grammar-legal JSON encoding of a single element.
        let ej = match element_json(elem) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let mut prefix = String::from("[");
        for j in 0..*hi {
            if j > 0 {
                prefix.push(',');
            }
            prefix.push_str(&ej);
        }
        let toks = match self.model.str_to_token(&prefix, AddBos::Never) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        // Advance the sampler across the full-length prefix. If any prefix token
        // is already forbidden, the tokenizer split the bytes across a grammar
        // boundary and the probe is inconclusive on this model.
        for t in &toks {
            let data = LlamaTokenData::new(*t, 1.0, 0.0);
            let mut arr = LlamaTokenDataArray::new(vec![data], false);
            sampler.apply(&mut arr);
            let logit = arr
                .data
                .first()
                .map(|d| d.logit())
                .unwrap_or(f32::NEG_INFINITY);
            if logit.is_infinite() && logit.is_sign_negative() {
                return Vec::new();
            }
            sampler.accept(*t);
        }
        let mut permitted = Vec::new();
        for cand in [",", "]"] {
            if !self.token_forbidden(&sampler, cand) {
                permitted.push(cand.to_string());
            }
        }
        permitted
    }
}

/// A grammar-legal JSON element encoding, matching what `grammar_to_gbnf`
/// admits, used to build a valid list prefix for the cardinality probe. Returns
/// `None` for shapes the probe does not synthesise (the probe then reports the
/// boundary as inconclusive rather than guessing).
fn element_json(g: &Grammar) -> Option<String> {
    match g {
        Grammar::Number { lo, .. } => Some(lo.to_string()),
        Grammar::Bool => Some("true".to_string()),
        Grammar::Text { .. } => Some("\"a\"".to_string()),
        Grammar::OneOf(variants) => {
            let v = variants.first()?;
            if v.fields.is_empty() {
                Some(format!("\"{}\"", v.name))
            } else {
                None
            }
        }
        Grammar::Record(fields) => {
            let mut parts = Vec::with_capacity(fields.len());
            for (n, sub) in fields {
                parts.push(format!("\"{n}\":{}", element_json(sub)?));
            }
            Some(format!("{{{}}}", parts.join(",")))
        }
        Grammar::List { .. } => None,
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
        let (text, confidence) = self.generate(req).unwrap_or_default();
        let value = json_to_value(&text, req.grammar)
            .or_else(|| scalar_from_text(&text, req.grammar))
            .unwrap_or_else(|| fallback_value(req.grammar));
        InferResult {
            value,
            confidence,
            provenance: self.provenance(req.intent_id, req.seed),
        }
    }

    fn infer_traced(&mut self, req: &InferRequest) -> (InferResult, Option<DecodeTrace>) {
        let permitted = self.permitted_first_step(req.grammar);
        let chosen = permitted.first().cloned().unwrap_or_default();
        let mut trace = vec![DecodeStep { permitted, chosen }];
        // For a bounded list, add the cardinality-boundary step so the
        // falsification harness can witness the COUNT BOUND masking (a strict
        // grammar forbids `,` after `hi` elements; a weakened larger bound still
        // permits it). For non-list grammars the first-step structure witness is
        // sufficient and this adds nothing.
        if matches!(req.grammar, Grammar::List { .. }) {
            let boundary = self.list_boundary_permitted(req.grammar);
            let chosen = boundary
                .iter()
                .find(|t| *t == "]")
                .cloned()
                .unwrap_or_else(|| boundary.first().cloned().unwrap_or_default());
            trace.push(DecodeStep {
                permitted: boundary,
                chosen,
            });
        }
        let result = self.infer(req);
        (result, Some(trace))
    }
}

/// The probability the grammar-masked distribution `arr` assigned to `token`:
/// a softmax over the surviving (finite-logit) candidates. This is exactly the
/// probability `LlamaSampler::dist` drew the token with, so it is a faithful
/// per-token confidence — not a re-derivation.
fn chosen_probability(arr: &LlamaTokenDataArray, token: llama_cpp_2::token::LlamaToken) -> f32 {
    let max = arr
        .data
        .iter()
        .map(|d| d.logit())
        .filter(|l| l.is_finite())
        .fold(f32::NEG_INFINITY, f32::max);
    if !max.is_finite() {
        return 0.0;
    }
    let mut sum = 0.0f32;
    let mut chosen = 0.0f32;
    for d in &arr.data {
        let l = d.logit();
        if l.is_finite() {
            let e = (l - max).exp();
            sum += e;
            if d.id() == token {
                chosen = e;
            }
        }
    }
    if sum > 0.0 {
        chosen / sum
    } else {
        0.0
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
