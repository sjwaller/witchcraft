//! The frontier (network) engine — a real API backend behind the same `Engine`
//! contract (change `add-inference-runtime`). It maps the output [`Grammar`] to a
//! JSON Schema and asks the provider for structured output.
//!
//! Litmus status is decided EMPIRICALLY by the falsification test, never by fiat.
//! Because a frontier provider enforces the schema server-side and exposes no
//! token-level mask, `infer_traced` returns no trace; the falsification harness
//! therefore cannot demonstrate masking and records this engine **non-litmus-safe**
//! with a reason. A litmus-strict `divine` site refuses it unless the source
//! carries an explicit downgrade (Verification A). It remains a fully usable
//! engine for needs that opt into the downgrade.
//!
//! Built only with `--features frontier` (links `ureq` + `serde_json`).

use crate::engine::{
    fallback_value, grammar_to_json_schema, json_to_value, Engine, EngineDescription, InferRequest,
    InferResult, LatencyClass, Locality, Modality,
};
use crate::manifest::{EngineSpec, NeedBinding};
use crate::value::Provenance;

pub struct FrontierEngine {
    backend_id: String,
    model: String,
    /// Resolved at request time from the environment, never source/manifest.
    api_key_env: String,
    endpoint: String,
}

impl FrontierEngine {
    pub fn from_spec(binding: &NeedBinding, spec: &EngineSpec) -> Self {
        let (backend_id, default_key, default_endpoint) = match spec.kind.as_str() {
            "openai" => (
                "frontier-openai",
                "OPENAI_API_KEY",
                "https://api.openai.com/v1/chat/completions",
            ),
            _ => (
                "frontier-anthropic",
                "ANTHROPIC_API_KEY",
                "https://api.anthropic.com/v1/messages",
            ),
        };
        FrontierEngine {
            backend_id: backend_id.to_string(),
            model: binding.model.clone(),
            api_key_env: spec
                .params
                .get("api_key_env")
                .cloned()
                .unwrap_or_else(|| default_key.to_string()),
            endpoint: spec
                .params
                .get("endpoint")
                .cloned()
                .unwrap_or_else(|| default_endpoint.to_string()),
        }
    }

    fn provenance(&self, intent_id: &str, seed: u64) -> Provenance {
        Provenance {
            oracle: intent_id.to_string(),
            model: self.model.clone(),
            model_version_or_sha: self.model.clone(),
            backend_id: self.backend_id.clone(),
            seed,
            sampling: "structured-output".to_string(),
        }
    }
}

impl Engine for FrontierEngine {
    fn describe(&self) -> EngineDescription {
        EngineDescription {
            backend_id: self.backend_id.clone(),
            tiers: vec!["frontier".to_string()],
            modalities: vec![Modality::Text],
            locality: Locality::Network,
            latency_class: LatencyClass::Batch,
            // It accepts a schema, but enforcement is server-side and not
            // observable as a token mask — so it is non-litmus-safe by default
            // until the falsification test proves otherwise.
            grammar_constrained: true,
            litmus_safe: Some(false),
            non_litmus_safe_reason: Some(
                "provider enforces JSON-schema structured output server-side; token-level \
                 masking is not observable, so masking-during-generation cannot be demonstrated"
                    .to_string(),
            ),
        }
    }

    fn infer(&mut self, req: &InferRequest) -> InferResult {
        let schema = grammar_to_json_schema(req.grammar);
        let api_key = std::env::var(&self.api_key_env).unwrap_or_default();

        let body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": req.input}],
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": "witchcraft_output", "schema": serde_json::from_str::<serde_json::Value>(&schema).unwrap_or(serde_json::Value::Null), "strict": true}
            }
        });

        let value = match call_provider(&self.endpoint, &api_key, &body) {
            Ok(resp) => json_to_value(&resp, req.grammar).unwrap_or_else(|| {
                if frontier_verbose() {
                    eprintln!("frontier: response did not parse; raw = {resp:?}");
                }
                fallback_value(req.grammar)
            }),
            Err(e) => {
                if frontier_verbose() {
                    eprintln!("frontier error: {e:?}");
                }
                fallback_value(req.grammar)
            }
        };

        InferResult {
            value,
            // Frontier APIs do not reliably expose per-token logprobs for
            // structured output; confidence is conservatively reported and the
            // sampling field records the mechanism (honest, not fabricated high).
            confidence: 1.0,
            provenance: self.provenance(req.intent_id, req.seed),
        }
    }

    // infer_traced uses the default (returns None): no token-level trace, so the
    // falsification test correctly marks this engine non-litmus-safe.
}

/// `true` when the user has opted into frontier request/response debugging via
/// `WITCHCRAFT_FRONTIER_VERBOSE=1` (or `true`). Default is quiet — the HTTP error
/// body is still threaded into the returned error, just not printed. Mirrors the
/// `WITCHCRAFT_LLAMA_VERBOSE` escape hatch.
fn frontier_verbose() -> bool {
    std::env::var("WITCHCRAFT_FRONTIER_VERBOSE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn call_provider(
    endpoint: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<String, String> {
    let resp = match ureq::post(endpoint)
        .set("authorization", &format!("Bearer {api_key}"))
        .set("content-type", "application/json")
        .send_json(body.clone())
    {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            return Err(format!("status {code}: {body}"));
        }
        Err(e) => return Err(e.to_string()),
    };
    let parsed: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    // OpenAI-style: choices[0].message.content holds the JSON string.
    parsed
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "provider response missing structured content".to_string())
}
