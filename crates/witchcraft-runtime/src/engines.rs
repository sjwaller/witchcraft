//! The compiled-path engine bridge (group 5): route a compiled `divine`/`embed`
//! through the *same* [`Engine`] contract + manifest the interpreter uses, so a
//! compiled program selects Mock / local llama / a frontier provider purely by
//! manifest, with zero source change — and refuses to start when a litmus-strict
//! need binds a non-litmus-safe engine.
//!
//! Models are named only in the manifest; the artifact carries only the semantic
//! intent and the output grammar (the litmus constraint). When no engine is
//! resolved for an intent (no manifest), the caller falls back to the built-in
//! deterministic Mock decoder — byte-identical to the interpreter's default.
//!
//! Only compiled with the `engines` feature, which pulls in `witchcraft`; the
//! self-contained `grimoire` staticlib is built without it and stays Mock-only.

use std::cell::RefCell;
use std::collections::HashMap;

use witchcraft::engine::{Engine, InferRequest, Policy};
use witchcraft::manifest::Manifest;
use witchcraft::value::Value as FeValue;

use crate::decode::Grammar as RtGrammar;
use crate::value::{self, Provenance, Value};

#[derive(Default)]
struct Bridge {
    manifest: Option<Manifest>,
    /// Engines resolved from the manifest at load, keyed by intent. Cached so
    /// every `divine` site for an intent shares one engine (one decode sequence),
    /// exactly as the interpreter does.
    engines: HashMap<String, Box<dyn Engine>>,
}

thread_local! {
    static BRIDGE: RefCell<Bridge> = RefCell::new(Bridge::default());
}

/// Install a manifest (parsed from a TOML subset). Clears any resolved engines.
pub fn set_manifest(src: &str) -> Result<(), String> {
    let manifest = Manifest::parse(src)?;
    BRIDGE.with(|b| {
        let mut b = b.borrow_mut();
        b.manifest = Some(manifest);
        b.engines.clear();
    });
    Ok(())
}

/// Drop the manifest and all resolved engines (the no-manifest / Mock default).
pub fn clear() {
    BRIDGE.with(|b| {
        let mut b = b.borrow_mut();
        b.manifest = None;
        b.engines.clear();
    });
}

/// Resolve every need against the manifest under its policy, caching the engines.
/// Returns the first refusal message (refuse-to-start) if any need cannot be
/// satisfied. With no manifest, this is a no-op (the Mock serves every need).
pub fn resolve_needs(needs: &[(String, bool, bool)]) -> Result<(), String> {
    let seed = crate::sink::seed();
    BRIDGE.with(|b| {
        let mut b = b.borrow_mut();
        b.engines.clear();
        let manifest = match &b.manifest {
            Some(m) => m.clone(),
            None => return Ok(()),
        };
        for (intent, allow_network, allow_downgrade) in needs {
            let policy = Policy {
                allow_network: *allow_network,
                allow_downgrade: *allow_downgrade,
                ..Policy::default()
            };
            match manifest.resolve(intent, &policy, seed) {
                Ok(engine) => {
                    b.engines.insert(intent.clone(), engine);
                }
                Err(e) => return Err(e.message()),
            }
        }
        Ok(())
    })
}

/// Run inference for `intent` through its resolved engine, if any. Returns the
/// produced value (top-provenance attached), confidence, and provenance. `None`
/// when no engine is resolved (the caller uses the built-in Mock decoder).
pub fn infer(intent: &str, grammar: &RtGrammar, input: &str) -> Option<(Value, f64, Provenance)> {
    let seed = crate::sink::seed();
    BRIDGE.with(|b| {
        let mut b = b.borrow_mut();
        let engine = b.engines.get_mut(intent)?;
        let fe_grammar = to_fe_grammar(grammar);
        let policy = Policy::default();
        let req = InferRequest {
            intent_id: intent,
            input,
            grammar: &fe_grammar,
            policy: &policy,
            seed,
        };
        let result = engine.infer(&req);
        let mut tags = HashMap::new();
        collect_tags(grammar, &mut tags);
        let prov = convert_provenance(&result.provenance);
        let value = convert_value(&result.value, &tags);
        let value = value::set_top_provenance(value, prov.clone());
        Some((value, result.confidence, prov))
    })
}

fn convert_provenance(p: &witchcraft::value::Provenance) -> Provenance {
    Provenance {
        oracle: p.oracle.clone(),
        model: p.model.clone(),
        model_version_or_sha: p.model_version_or_sha.clone(),
        backend_id: p.backend_id.clone(),
        seed: p.seed,
        sampling: p.sampling.clone(),
    }
}

/// Convert a front-end value into a runtime heap value, assigning each variant
/// the interned tag the artifact's grammar carries (so a decoded variant
/// dispatches correctly through a compiled `enact`).
fn convert_value(v: &FeValue, tags: &HashMap<String, u32>) -> Value {
    match v {
        FeValue::Spark(n) => value::spark(*n),
        FeValue::Bool(b) => value::boolean(*b),
        FeValue::Glyph(s) => value::glyph(s),
        FeValue::Unit => value::unit(),
        FeValue::Record { fields, .. } => {
            let f = fields
                .iter()
                .map(|(n, c)| (n.clone(), convert_value(c, tags)))
                .collect();
            value::record(f, None)
        }
        FeValue::Variant { name, fields, .. } => {
            let tag = tags.get(name).copied().unwrap_or(u32::MAX);
            let f = fields
                .iter()
                .map(|(n, c)| (n.clone(), convert_value(c, tags)))
                .collect();
            value::variant(name, tag, f, None)
        }
        FeValue::List(items) => value::list(items.iter().map(|c| convert_value(c, tags)).collect()),
        FeValue::Embedding { space, vector, .. } => value::embedding(space, vector.clone(), None),
        FeValue::Inferred {
            inner,
            confidence,
            provenance,
        } => value::inferred(
            convert_value(inner, tags),
            *confidence,
            convert_provenance(provenance),
        ),
        // Oracles are never produced by inference.
        FeValue::Oracle { .. } => value::unit(),
    }
}

fn collect_tags(g: &RtGrammar, out: &mut HashMap<String, u32>) {
    match g {
        RtGrammar::Record(fields) => {
            for (_, sub) in fields {
                collect_tags(sub, out);
            }
        }
        RtGrammar::OneOf(variants) => {
            for v in variants {
                out.insert(v.name.clone(), v.tag);
                for (_, sub) in &v.fields {
                    collect_tags(sub, out);
                }
            }
        }
        _ => {}
    }
}

fn to_fe_grammar(g: &RtGrammar) -> witchcraft::grammar::Grammar {
    use witchcraft::grammar::{Grammar as Fe, GrammarVariant as FeVar};
    match g {
        RtGrammar::Number { lo, hi } => Fe::Number { lo: *lo, hi: *hi },
        RtGrammar::Bool => Fe::Bool,
        RtGrammar::Text { max_len } => Fe::Text { max_len: *max_len },
        RtGrammar::Record(fields) => Fe::Record(
            fields
                .iter()
                .map(|(n, sub)| (n.clone(), to_fe_grammar(sub)))
                .collect(),
        ),
        RtGrammar::OneOf(variants) => Fe::OneOf(
            variants
                .iter()
                .map(|v| FeVar {
                    name: v.name.clone(),
                    fields: v
                        .fields
                        .iter()
                        .map(|(n, sub)| (n.clone(), to_fe_grammar(sub)))
                        .collect(),
                })
                .collect(),
        ),
    }
}
