//! BINDING — the deployment manifest and load-time resolution (change
//! `add-inference-runtime`). The manifest is the *only* place a model is named;
//! application source names only a semantic intent. At load, each need is
//! resolved against the manifest and checked against the program's POLICY
//! (locality vs `permit(network)`, litmus-strictness). A need that cannot be
//! satisfied under the policy makes the program **refuse to start** — it never
//! silently crosses a policy boundary.
//!
//! "Same program, different manifest" = laptop / edge / GPU / cloud, with no
//! source change. The manifest is a small TOML subset parsed without an external
//! dependency (the toolchain stays self-contained).

use std::collections::HashMap;

use crate::engine::mock::MockEngine;
use crate::engine::{Engine, Locality, Policy};

/// How an intent resolves to a concrete engine + model in this deployment.
#[derive(Clone, Debug)]
pub struct NeedBinding {
    pub engine: String,
    pub model: String,
    pub sha256: Option<String>,
    pub locality: Option<Locality>,
    pub latency: Option<String>,
    pub tier: Option<String>,
}

/// A concrete engine definition (models named only here).
#[derive(Clone, Debug)]
pub struct EngineSpec {
    pub kind: String,
    pub params: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub needs: HashMap<String, NeedBinding>,
    pub engines: HashMap<String, EngineSpec>,
}

/// Why a need could not be bound under the policy — every variant refuses to
/// start with a precise, source-auditable message.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    UnknownNeed(String),
    UnknownEngine {
        need: String,
        engine: String,
    },
    NetworkNotPermitted {
        need: String,
        engine: String,
    },
    NonLitmusSafe {
        need: String,
        engine: String,
        reason: String,
    },
    EngineUnavailable {
        need: String,
        engine: String,
        kind: String,
    },
}

impl ResolveError {
    pub fn message(&self) -> String {
        match self {
            ResolveError::UnknownNeed(n) => format!(
                "refuse to start: need `{n}` has no binding in the manifest (every inference need must resolve to an engine)"
            ),
            ResolveError::UnknownEngine { need, engine } => format!(
                "refuse to start: need `{need}` is bound to engine `{engine}`, which the manifest does not define"
            ),
            ResolveError::NetworkNotPermitted { need, engine } => format!(
                "refuse to start: need `{need}` is bound to network engine `{engine}` but its site does not grant `permit(network)` (on-device-only is the default)"
            ),
            ResolveError::NonLitmusSafe { need, engine, reason } => format!(
                "refuse to start: need `{need}` is litmus-strict but engine `{engine}` is non-litmus-safe ({reason}); add a source-visible downgrade to run anyway"
            ),
            ResolveError::EngineUnavailable { need, engine, kind } => format!(
                "refuse to start: need `{need}` is bound to engine `{engine}` of kind `{kind}`, which is not compiled into this build (enable the corresponding feature)"
            ),
        }
    }
}

impl Manifest {
    /// Parse a manifest from a TOML subset: `[need.<intent>]` / `[engine.<id>]`
    /// tables with `key = "value"` string entries. Unknown keys are ignored;
    /// comments (`#`) and blank lines are skipped.
    pub fn parse(src: &str) -> Result<Manifest, String> {
        let mut needs: HashMap<String, NeedBinding> = HashMap::new();
        let mut engines: HashMap<String, EngineSpec> = HashMap::new();
        let mut raw_needs: HashMap<String, HashMap<String, String>> = HashMap::new();

        enum Section {
            None,
            Need(String),
            Engine(String),
        }
        let mut section = Section::None;

        for (lineno, raw) in src.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(header) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let header = header.trim();
                if let Some(name) = header.strip_prefix("need.") {
                    section = Section::Need(name.trim().to_string());
                    raw_needs.entry(name.trim().to_string()).or_default();
                } else if let Some(name) = header.strip_prefix("engine.") {
                    section = Section::Engine(name.trim().to_string());
                    engines
                        .entry(name.trim().to_string())
                        .or_insert(EngineSpec {
                            kind: String::new(),
                            params: HashMap::new(),
                        });
                } else {
                    return Err(format!(
                        "manifest line {}: unknown table `[{}]` (expected [need.*] or [engine.*])",
                        lineno + 1,
                        header
                    ));
                }
                continue;
            }
            let (key, value) = parse_kv(line).ok_or_else(|| {
                format!("manifest line {}: expected `key = \"value\"`", lineno + 1)
            })?;
            match &section {
                Section::Need(name) => {
                    raw_needs.get_mut(name).unwrap().insert(key, value);
                }
                Section::Engine(name) => {
                    let spec = engines.get_mut(name).unwrap();
                    if key == "kind" {
                        spec.kind = value;
                    } else {
                        spec.params.insert(key, value);
                    }
                }
                Section::None => {
                    return Err(format!(
                        "manifest line {}: key outside any [need.*]/[engine.*] table",
                        lineno + 1
                    ));
                }
            }
        }

        for (name, kv) in raw_needs {
            let engine = kv
                .get("engine")
                .cloned()
                .ok_or_else(|| format!("manifest need `{name}` is missing `engine = \"...\"`"))?;
            let model = kv.get("model").cloned().unwrap_or_default();
            let locality = match kv.get("locality").map(|s| s.as_str()) {
                Some("local") => Some(Locality::Local),
                Some("network") => Some(Locality::Network),
                Some(other) => {
                    return Err(format!(
                        "manifest need `{name}`: unknown locality `{other}` (use local|network)"
                    ))
                }
                None => None,
            };
            needs.insert(
                name,
                NeedBinding {
                    engine,
                    model,
                    sha256: kv.get("sha256").cloned(),
                    locality,
                    latency: kv.get("latency").cloned(),
                    tier: kv.get("tier").cloned(),
                },
            );
        }

        Ok(Manifest { needs, engines })
    }

    /// Resolve a single intent under `policy` into a live engine, or refuse to
    /// start. `seed` seeds deterministic engines (the Mock).
    pub fn resolve(
        &self,
        intent: &str,
        policy: &Policy,
        seed: u64,
    ) -> Result<Box<dyn Engine>, ResolveError> {
        let binding = self
            .needs
            .get(intent)
            .ok_or_else(|| ResolveError::UnknownNeed(intent.to_string()))?;
        let spec =
            self.engines
                .get(&binding.engine)
                .ok_or_else(|| ResolveError::UnknownEngine {
                    need: intent.to_string(),
                    engine: binding.engine.clone(),
                })?;
        build_engine(intent, binding, spec, policy, seed)
    }
}

fn strip_comment(line: &str) -> &str {
    // No `#` inside our values (ids/paths), so a first-`#` split is sufficient.
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn parse_kv(line: &str) -> Option<(String, String)> {
    let eq = line.find('=')?;
    let key = line[..eq].trim().to_string();
    let mut val = line[eq + 1..].trim();
    if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
        || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
    {
        val = &val[1..val.len() - 1];
    }
    if key.is_empty() {
        return None;
    }
    Some((key, val.to_string()))
}

/// Construct a live engine for a binding, enforcing the policy (refuse-to-start).
fn build_engine(
    intent: &str,
    binding: &NeedBinding,
    spec: &EngineSpec,
    policy: &Policy,
    seed: u64,
) -> Result<Box<dyn Engine>, ResolveError> {
    match spec.kind.as_str() {
        "mock" | "" => {
            // The Mock is local + litmus-safe; it always satisfies the policy.
            let model = if binding.model.is_empty() {
                intent.to_string()
            } else {
                binding.model.clone()
            };
            Ok(Box::new(MockEngine::new(seed, model)))
        }
        "llama" | "llama-cpp" => build_llama(intent, binding, spec, policy),
        "anthropic" | "openai" | "frontier" => build_frontier(intent, binding, spec, policy),
        other => Err(ResolveError::EngineUnavailable {
            need: intent.to_string(),
            engine: binding.engine.clone(),
            kind: other.to_string(),
        }),
    }
}

#[cfg(feature = "llama")]
fn build_llama(
    intent: &str,
    binding: &NeedBinding,
    spec: &EngineSpec,
    policy: &Policy,
) -> Result<Box<dyn Engine>, ResolveError> {
    // Local engine — always satisfies on-device-only. llama.cpp is litmus-safe
    // (GBNF token masking), so no litmus refusal.
    let _ = policy;
    let engine = crate::engine::llama::LlamaEngine::from_spec(binding, spec).map_err(|reason| {
        ResolveError::NonLitmusSafe {
            need: intent.to_string(),
            engine: binding.engine.clone(),
            reason,
        }
    })?;
    Ok(Box::new(engine))
}

#[cfg(not(feature = "llama"))]
fn build_llama(
    intent: &str,
    binding: &NeedBinding,
    _spec: &EngineSpec,
    _policy: &Policy,
) -> Result<Box<dyn Engine>, ResolveError> {
    Err(ResolveError::EngineUnavailable {
        need: intent.to_string(),
        engine: binding.engine.clone(),
        kind: "llama".to_string(),
    })
}

#[cfg(feature = "frontier")]
fn build_frontier(
    intent: &str,
    binding: &NeedBinding,
    spec: &EngineSpec,
    policy: &Policy,
) -> Result<Box<dyn Engine>, ResolveError> {
    use crate::engine::Engine as _;
    // Network engine — requires permit(network).
    if !policy.allow_network {
        return Err(ResolveError::NetworkNotPermitted {
            need: intent.to_string(),
            engine: binding.engine.clone(),
        });
    }
    let engine = crate::engine::frontier::FrontierEngine::from_spec(binding, spec);
    let desc = engine.describe();
    if policy.litmus_strict && desc.litmus_safe == Some(false) && !policy.allow_downgrade {
        return Err(ResolveError::NonLitmusSafe {
            need: intent.to_string(),
            engine: binding.engine.clone(),
            reason: desc
                .non_litmus_safe_reason
                .unwrap_or_else(|| "engine does not mask tokens during generation".to_string()),
        });
    }
    Ok(Box::new(engine))
}

#[cfg(not(feature = "frontier"))]
fn build_frontier(
    intent: &str,
    binding: &NeedBinding,
    _spec: &EngineSpec,
    policy: &Policy,
) -> Result<Box<dyn Engine>, ResolveError> {
    // Even without the feature, enforce the policy boundary first so the error a
    // user sees is the *policy* violation, not a build-availability detail.
    if !policy.allow_network {
        return Err(ResolveError::NetworkNotPermitted {
            need: intent.to_string(),
            engine: binding.engine.clone(),
        });
    }
    Err(ResolveError::EngineUnavailable {
        need: intent.to_string(),
        engine: binding.engine.clone(),
        kind: "frontier".to_string(),
    })
}
