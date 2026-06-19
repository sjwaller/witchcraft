//! The pluggable inference contract (change `add-inference-runtime`).
//!
//! Inference is a *swappable engine* the program is written against, never a
//! model it is bound to. A program names a NEED (an oracle intent + an output
//! grammar) and a POLICY (locality, litmus-strictness); a deployment manifest
//! BINDS that need to a concrete [`Engine`]. The language trusts the *contract*,
//! not any engine: the universal property every legal engine must satisfy is
//! grammar-by-construction — the output grammar constrains generation
//! token-by-token, so illegal outputs are unreachable (never validate-after).
//!
//! Only the `Mock` engine is built unconditionally (it is the offline default
//! and the deterministic litmus oracle). Real engines live behind cargo features
//! so the default build/test stays self-contained and offline.

use crate::grammar::Grammar;
use crate::value::{Provenance, Value};

/// Where an engine runs. On-device-only is the default policy; a network engine
/// is eligible only when the need's site grants `permit(network)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locality {
    Local,
    Network,
}

/// A coarse latency hint used in matching/ranking (not a benchmark).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatencyClass {
    Interactive,
    Batch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modality {
    Text,
}

/// What an engine advertises about itself. `grammar_constrained` is mandatory:
/// an engine that cannot enforce the grammar *during* generation is not a legal
/// engine for a litmus-strict need. `litmus_safe` is the empirically-determined
/// result of the falsification test (set to `false`, with a reason, when an
/// engine only validates-after rather than masking tokens).
#[derive(Clone, Debug)]
pub struct EngineDescription {
    pub backend_id: String,
    /// Advisory only — used to *prefer* engines, never to gate. No metric is
    /// defined here (see proposal open questions); this is an engine-declared
    /// string such as `"standard"`.
    pub tiers: Vec<String>,
    pub modalities: Vec<Modality>,
    pub locality: Locality,
    pub latency_class: LatencyClass,
    /// MANDATORY. `false` ⇒ not a legal engine for a grammar-constrained need.
    pub grammar_constrained: bool,
    /// Whether the falsification test has shown this engine masks tokens during
    /// generation. `None` until determined; `Some(false)` marks it non-litmus-safe.
    pub litmus_safe: Option<bool>,
    /// Recorded reason when `litmus_safe == Some(false)`.
    pub non_litmus_safe_reason: Option<String>,
}

/// The author-stated constraints for a need (source-visible, §9.1). Constraints,
/// not engine choices.
#[derive(Clone, Debug)]
pub struct Policy {
    /// `permit(network)` granted at the need's site.
    pub allow_network: bool,
    /// Hard latency hint (matching), if stated.
    pub latency: Option<LatencyClass>,
    /// Advisory minimum tier — preference only, never gates (§ tier-is-advisory).
    pub min_tier: Option<String>,
    /// A `divine` site is litmus-strict by default.
    pub litmus_strict: bool,
    /// Source-visible acknowledgement that the author accepts running this need
    /// on a non-litmus-safe engine. Absent ⇒ a non-litmus-safe binding refuses.
    pub allow_downgrade: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            allow_network: false,
            latency: None,
            min_tier: None,
            litmus_strict: true,
            allow_downgrade: false,
        }
    }
}

/// A single inference request. The `input` (the prompt the v0.1 ABI dropped) is
/// threaded to the engine; the `grammar` constrains generation.
pub struct InferRequest<'a> {
    pub intent_id: &'a str,
    pub input: &'a str,
    pub grammar: &'a Grammar,
    pub policy: &'a Policy,
    pub seed: u64,
}

/// The result of an inference. `value` inhabits `grammar` by construction.
pub struct InferResult {
    pub value: Value,
    pub confidence: f64,
    pub provenance: Provenance,
}

/// One decode step's evidence that the grammar was live during generation: the
/// set of next-tokens the grammar *permitted* at this step (and the one chosen).
/// The falsification test asserts that, at one or more steps, a token the
/// weakened grammar permitted was *forbidden* by the real grammar — i.e. masking
/// actually occurred, rather than comparing final outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodeStep {
    pub permitted: Vec<String>,
    pub chosen: String,
}

pub type DecodeTrace = Vec<DecodeStep>;

/// The contract every inference backend implements.
pub trait Engine {
    fn describe(&self) -> EngineDescription;

    /// Generate a value inhabiting `req.grammar` by construction, with a
    /// confidence and provenance produced by the engine (never synthesised).
    fn infer(&mut self, req: &InferRequest) -> InferResult;

    /// Generate, additionally returning the per-step permitted-token trace so the
    /// falsification test can prove masking occurred. An engine that cannot
    /// expose a token-level trace returns `None` and is judged by the strongest
    /// available evidence (see the falsification harness).
    fn infer_traced(&mut self, req: &InferRequest) -> (InferResult, Option<DecodeTrace>) {
        (self.infer(req), None)
    }

    /// Produce an embedding of `input` tagged with `space`. Consumed by the
    /// `complete-native-compile` change for compiled `oracle.embed`.
    fn embed(&mut self, _intent_id: &str, input: &str, space: &str) -> Value {
        // Default: the deterministic hash embedding (the Mock behaviour),
        // shared so every engine has a usable embed unless it overrides it.
        crate::engine::mock::embed_hash(input, space)
    }
}

pub mod mock;

#[cfg(feature = "llama")]
pub mod llama;

#[cfg(feature = "frontier")]
pub mod frontier;

/// Parse a JSON document (a provider response or a grammar-constrained
/// generation) into a Witchcraft value that inhabits `grammar`. Shared by the
/// real engines; only built when one of them is enabled.
#[cfg(any(feature = "llama", feature = "frontier"))]
pub fn json_to_value(text: &str, grammar: &Grammar) -> Option<Value> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    json_convert(&json, grammar)
}

#[cfg(any(feature = "llama", feature = "frontier"))]
fn json_convert(json: &serde_json::Value, grammar: &Grammar) -> Option<Value> {
    match grammar {
        Grammar::Number { .. } => json.as_i64().map(|n| Value::Spark(n as f64)),
        Grammar::Bool => json.as_bool().map(Value::Bool),
        Grammar::Text { .. } => json.as_str().map(|s| Value::Glyph(s.to_string())),
        Grammar::Record(fields) => {
            let mut out = Vec::with_capacity(fields.len());
            for (name, sub) in fields {
                out.push((name.clone(), json_convert(json.get(name)?, sub)?));
            }
            Some(Value::Record {
                fields: out,
                provenance: None,
            })
        }
        Grammar::OneOf(variants) => {
            // Bare variant as a plain string (llama GBNF form: `"Name"`).
            if let Some(name) = json.as_str() {
                let v = variants
                    .iter()
                    .find(|v| v.name == name && v.fields.is_empty())?;
                return Some(Value::Variant {
                    name: v.name.clone(),
                    fields: vec![],
                    provenance: None,
                });
            }
            // Discriminator-tagged object (frontier OpenAI-strict form:
            // `{"tag":"Name", <payload fields…>}`). Payload fields are siblings
            // of `tag`, matching `grammar_to_json_schema`'s `anyOf` branches.
            if let Some(tag) = json.get("tag").and_then(|t| t.as_str()) {
                let v = variants.iter().find(|v| v.name == tag)?;
                let mut out = Vec::with_capacity(v.fields.len());
                for (fname, fsub) in &v.fields {
                    out.push((fname.clone(), json_convert(json.get(fname)?, fsub)?));
                }
                return Some(Value::Variant {
                    name: v.name.clone(),
                    fields: out,
                    provenance: None,
                });
            }
            // Nested-key object (llama GBNF form: `{"Name":{ fields }}`).
            for v in variants {
                if let Some(obj) = json.get(&v.name) {
                    let mut out = Vec::with_capacity(v.fields.len());
                    for (fname, fsub) in &v.fields {
                        out.push((fname.clone(), json_convert(obj.get(fname)?, fsub)?));
                    }
                    return Some(Value::Variant {
                        name: v.name.clone(),
                        fields: out,
                        provenance: None,
                    });
                }
            }
            None
        }
        Grammar::List { elem, .. } => {
            let arr = json.as_array()?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(json_convert(item, elem)?);
            }
            Some(Value::List(out))
        }
    }
}

/// A grammar-inhabiting default — the value is in-type by construction even when
/// a real engine's transport fails (shape guaranteed regardless, §8).
#[cfg(any(feature = "llama", feature = "frontier"))]
pub fn fallback_value(grammar: &Grammar) -> Value {
    match grammar {
        Grammar::Number { lo, .. } => Value::Spark(*lo as f64),
        Grammar::Bool => Value::Bool(false),
        Grammar::Text { .. } => Value::Glyph(String::new()),
        Grammar::Record(fields) => Value::Record {
            fields: fields
                .iter()
                .map(|(n, g)| (n.clone(), fallback_value(g)))
                .collect(),
            provenance: None,
        },
        Grammar::OneOf(variants) => {
            let v = &variants[0];
            Value::Variant {
                name: v.name.clone(),
                fields: v
                    .fields
                    .iter()
                    .map(|(n, g)| (n.clone(), fallback_value(g)))
                    .collect(),
                provenance: None,
            }
        }
        // The minimum legal cardinality keeps the value in-type by construction.
        Grammar::List { elem, lo, .. } => {
            Value::List((0..*lo).map(|_| fallback_value(elem)).collect())
        }
    }
}

/// The outcome of the falsification test against one engine (Verification B).
/// `masked` is the headline: it is `true` only when, at one or more decode steps,
/// a token the *weakened* grammar permits was *forbidden* by the *real* grammar —
/// proving the type constrained generation (masking occurred), not that the
/// final outputs merely differ. `witness` records that step for diagnostics.
#[derive(Clone, Debug)]
pub struct Falsification {
    pub backend_id: String,
    pub masked: bool,
    pub witness: Option<FalsifyWitness>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FalsifyWitness {
    pub step: usize,
    /// A token the weakened grammar permitted at this step but the real grammar
    /// forbade.
    pub forbidden_token: String,
}

/// Run the falsification test against an engine: build the same `divine` site
/// once with the real grammar and once with the weakened grammar, drive the
/// engine through both, and inspect the per-step *permitted-token* traces to
/// assert masking occurred. An engine that exposes no token trace cannot
/// demonstrate masking and is judged not litmus-safe by this harness (the caller
/// records the reason). Comparing final outputs is deliberately *not* used.
pub fn falsify(
    engine: &mut dyn Engine,
    intent_id: &str,
    input: &str,
    real: &Grammar,
    weakened: &Grammar,
    seed: u64,
) -> Falsification {
    let backend_id = engine.describe().backend_id;
    let policy = Policy::default();

    let (_r, real_trace) = engine.infer_traced(&InferRequest {
        intent_id,
        input,
        grammar: real,
        policy: &policy,
        seed,
    });
    let (_w, weak_trace) = engine.infer_traced(&InferRequest {
        intent_id,
        input,
        grammar: weakened,
        policy: &policy,
        seed,
    });

    let (real_trace, weak_trace) = match (real_trace, weak_trace) {
        (Some(r), Some(w)) => (r, w),
        _ => {
            return Falsification {
                backend_id,
                masked: false,
                witness: None,
                reason: "engine exposes no token-level decode trace; masking during \
                         generation cannot be demonstrated (treat as non-litmus-safe)"
                    .to_string(),
            };
        }
    };

    // Find a step where the weakened grammar permitted a token the real grammar
    // forbade. This is the proof that the type masked the logits during
    // generation rather than being validated afterwards.
    let steps = real_trace.len().min(weak_trace.len()).max(1);
    for step in 0..steps {
        let real_permitted = real_trace.get(step).map(|s| &s.permitted);
        let weak_permitted = weak_trace.get(step).map(|s| &s.permitted);
        if let (Some(realp), Some(weakp)) = (real_permitted, weak_permitted) {
            if let Some(tok) = weakp.iter().find(|t| !realp.contains(t)) {
                return Falsification {
                    backend_id,
                    masked: true,
                    witness: Some(FalsifyWitness {
                        step,
                        forbidden_token: tok.clone(),
                    }),
                    reason: format!(
                        "litmus holds: at step {step} the real grammar forbade token `{tok}` \
                         which the weakened grammar permitted (masking occurred during generation)"
                    ),
                };
            }
        }
    }

    Falsification {
        backend_id,
        masked: false,
        witness: None,
        reason: "LITMUS FAILED: real and weakened grammars permitted the same tokens at every \
                 step — the type did not participate in generation; this engine is a wrapper, \
                 not AI-first"
            .to_string(),
    }
}

/// Compile a Witchcraft [`Grammar`] into GBNF — the grammar format llama.cpp
/// enforces token-by-token during generation. This is the bridge that makes the
/// local engine litmus-safe: the *same* output type that the Mock walks becomes a
/// hard constraint the real decoder masks against. Pure and testable without the
/// `llama` feature so the litmus-critical mapping is always covered.
pub fn grammar_to_gbnf(grammar: &Grammar) -> String {
    // The whole grammar inlines into the `root` rule (closed alternations and
    // fixed-key objects), so no helper rules are needed.
    let body = match grammar {
        // A bare (root) text output is the weakened / "no type" form: emit
        // UNQUOTED free prose (a `divine x: glyph` is parsed by `scalar_from_text`,
        // and this is the open set the litmus contrasts against). Nested `glyph`
        // fields, by contrast, are quoted JSON strings (see `gbnf_rule`) so the
        // enclosing object/array stays valid JSON and round-trips through
        // `json_to_value`.
        Grammar::Text { max_len } => format!("({})", text_char_classes(*max_len)),
        _ => gbnf_rule(grammar),
    };
    format!("root ::= {body}\n")
}

/// The character class a bounded `glyph` admits, length-capped via a single GBNF
/// bounded repetition `[class]{0,N}` (NOT N chained `char?` slots — that made the
/// grammar engine fan out O(N) parallel stacks per token and dominated decode
/// time). `{0,N}` compiles to a recursive `S'(k) ::= class S'(k-1) |` chain, so
/// only one level is active per step and per-token cost is independent of `N`.
/// The class excludes `"` and `\`, so a quoted JSON string built from it needs no
/// escaping. Still a hard upper bound: a glyph longer than `N` is unreachable.
fn text_char_classes(max_len: usize) -> String {
    format!("[a-zA-Z0-9 .,!?]{{0,{}}}", max_len.max(1))
}

fn gbnf_rule(grammar: &Grammar) -> String {
    match grammar {
        Grammar::Number { lo, hi } => {
            // Closed integer alternation in [lo, hi] — illegal numbers are
            // unreachable (no out-of-range token sequence is generable).
            let alts: Vec<String> = (*lo..=*hi).map(|n| format!("\"{n}\"")).collect();
            format!("({})", alts.join(" | "))
        }
        Grammar::Bool => "(\"true\" | \"false\")".to_string(),
        Grammar::Text { max_len } => {
            // A JSON STRING value: the bounded text wrapped in literal double
            // quotes, so a glyph field inside a record/list produces valid JSON
            // (`"narration":"..."`, not `"narration":...`). Without the quotes the
            // enclosing object fails to parse and the whole record decodes to its
            // empty fallback — the dungeon-master "every field empty" bug.
            format!("(\"\\\"\" {} \"\\\"\")", text_char_classes(*max_len))
        }
        Grammar::Record(fields) => {
            // A JSON-ish object with fixed keys in order.
            let mut parts: Vec<String> = Vec::new();
            parts.push("\"{\"".to_string());
            for (i, (name, sub)) in fields.iter().enumerate() {
                if i > 0 {
                    parts.push("\",\"".to_string());
                }
                let sub_rule = gbnf_rule(sub);
                parts.push(format!("\"\\\"{name}\\\":\" {sub_rule}"));
            }
            parts.push("\"}\"".to_string());
            format!("({})", parts.join(" "))
        }
        Grammar::OneOf(variants) => {
            // Closed alternation over exactly the declared variant names.
            let alts: Vec<String> = variants
                .iter()
                .map(|v| {
                    if v.fields.is_empty() {
                        format!("\"\\\"{}\\\"\"", v.name)
                    } else {
                        let mut parts = vec![format!("\"{{\\\"{}\\\":{{\"", v.name)];
                        for (i, (fname, fsub)) in v.fields.iter().enumerate() {
                            if i > 0 {
                                parts.push("\",\"".to_string());
                            }
                            let sub_rule = gbnf_rule(fsub);
                            parts.push(format!("\"\\\"{fname}\\\":\" {sub_rule}"));
                        }
                        parts.push("\"}}\"".to_string());
                        format!("({})", parts.join(" "))
                    }
                })
                .collect();
            format!("({})", alts.join(" | "))
        }
        Grammar::List { elem, lo, hi } => {
            // Design D3 (approach A): expand the bounded list into a closed
            // disjunction of fixed-length JSON arrays for each legal cardinality
            // lo..=hi. There is NO unbounded `*` repetition — an over-length list
            // is simply not a member of the alternation, so llama.cpp's GBNF
            // sampler masks it token-by-token (litmus-safe). hi is capped at
            // compile time (LIST_MAX_HI) to keep this expansion small.
            let elem_rule = gbnf_rule(elem);
            let alts: Vec<String> = (*lo..=*hi)
                .map(|k| {
                    if k == 0 {
                        "\"[]\"".to_string()
                    } else {
                        let mut parts = vec!["\"[\"".to_string()];
                        for j in 0..k {
                            if j > 0 {
                                parts.push("\",\"".to_string());
                            }
                            parts.push(format!("({elem_rule})"));
                        }
                        parts.push("\"]\"".to_string());
                        format!("({})", parts.join(" "))
                    }
                })
                .collect();
            format!("({})", alts.join(" | "))
        }
    }
}

/// Compile a Witchcraft [`Grammar`] into a JSON Schema — the structured-output
/// format a frontier provider enforces. Emits the **OpenAI strict** dialect:
/// every object lists all of its properties in `required` and sets
/// `additionalProperties: false`, and a variant (`one_of`) is an `anyOf` of
/// discriminator-tagged objects (`{"tag":"<Variant>", <payload…>}`) rather than
/// `oneOf` (which OpenAI strict mode forbids). Whether the provider enforces it
/// *during* generation (litmus-safe) or only validates-after is decided
/// empirically by the falsification test, not by this mapping. Pure and testable.
pub fn grammar_to_json_schema(grammar: &Grammar) -> String {
    json_schema_strict(grammar)
}

/// A closed, strict JSON-Schema object from `(name, sub-schema)` pairs: every
/// property is `required` and `additionalProperties` is `false` — what OpenAI
/// strict mode demands of *every* object in the schema.
fn strict_object(props: &[(String, String)]) -> String {
    let props_str: Vec<String> = props.iter().map(|(n, s)| format!("\"{n}\":{s}")).collect();
    let required: Vec<String> = props.iter().map(|(n, _)| format!("\"{n}\"")).collect();
    format!(
        "{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}],\"additionalProperties\":false}}",
        props_str.join(","),
        required.join(",")
    )
}

fn json_schema_strict(grammar: &Grammar) -> String {
    match grammar {
        Grammar::Number { lo, hi } => {
            format!("{{\"type\":\"integer\",\"minimum\":{lo},\"maximum\":{hi}}}")
        }
        Grammar::Bool => "{\"type\":\"boolean\"}".to_string(),
        Grammar::Text { max_len } => {
            format!("{{\"type\":\"string\",\"maxLength\":{max_len}}}")
        }
        Grammar::Record(fields) => {
            let props: Vec<(String, String)> = fields
                .iter()
                .map(|(n, g)| (n.clone(), json_schema_strict(g)))
                .collect();
            strict_object(&props)
        }
        Grammar::OneOf(variants) => {
            // `anyOf` of discriminator-tagged closed objects. A bare variant is
            // `{tag:{const:"Name"}}`; a payload variant adds its fields. Every
            // branch is a strict object (all props required, no extras), so the
            // whole union satisfies OpenAI strict mode (which rejects `oneOf`).
            let branches: Vec<String> = variants
                .iter()
                .map(|v| {
                    let mut props: Vec<(String, String)> =
                        vec![("tag".to_string(), format!("{{\"const\":\"{}\"}}", v.name))];
                    for (fname, fsub) in &v.fields {
                        props.push((fname.clone(), json_schema_strict(fsub)));
                    }
                    strict_object(&props)
                })
                .collect();
            format!("{{\"anyOf\":[{}]}}", branches.join(","))
        }
        Grammar::List { elem, lo, hi } => {
            // A bounded JSON array. NOTE: `minItems`/`maxItems` is the schema's
            // *validate-after* expression of the bound. Whether the provider
            // honours it DURING generation (litmus-safe) or only checks it after
            // is decided empirically by the falsification harness — which marks
            // the frontier engine non-litmus-safe because it exposes no
            // token-level mask. So this schema does not, by itself, make a
            // bounded list litmus-safe on a network provider.
            format!(
                "{{\"type\":\"array\",\"items\":{},\"minItems\":{lo},\"maxItems\":{hi}}}",
                json_schema_strict(elem)
            )
        }
    }
}

/// A JSON-Schema sketch for the *llama* system prompt — describes the shape the
/// GBNF actually emits (a bare variant is the string `"Name"`, a payload variant
/// the nested object `{"Name":{…}}`), so the local model is guided toward the
/// form its grammar mask enforces. Distinct from [`grammar_to_json_schema`],
/// whose OpenAI-strict tagged form is a *different* wire shape; using the strict
/// schema here would describe a layout the GBNF forbids. Prompt-only guidance.
#[cfg(feature = "llama")]
pub fn grammar_to_prompt_schema(grammar: &Grammar) -> String {
    json_schema_gbnf_shaped(grammar)
}

#[cfg(feature = "llama")]
fn json_schema_gbnf_shaped(grammar: &Grammar) -> String {
    match grammar {
        Grammar::Number { lo, hi } => {
            format!("{{\"type\":\"integer\",\"minimum\":{lo},\"maximum\":{hi}}}")
        }
        Grammar::Bool => "{\"type\":\"boolean\"}".to_string(),
        Grammar::Text { max_len } => {
            format!("{{\"type\":\"string\",\"maxLength\":{max_len}}}")
        }
        Grammar::Record(fields) => {
            let props: Vec<String> = fields
                .iter()
                .map(|(n, g)| format!("\"{n}\":{}", json_schema_gbnf_shaped(g)))
                .collect();
            format!(
                "{{\"type\":\"object\",\"properties\":{{{}}}}}",
                props.join(",")
            )
        }
        Grammar::OneOf(variants) => {
            let simple: Vec<&str> = variants
                .iter()
                .filter(|v| v.fields.is_empty())
                .map(|v| v.name.as_str())
                .collect();
            if simple.len() == variants.len() {
                let names: Vec<String> = simple.iter().map(|n| format!("\"{n}\"")).collect();
                format!("{{\"enum\":[{}]}}", names.join(","))
            } else {
                let alts: Vec<String> = variants
                    .iter()
                    .map(|v| {
                        let props: Vec<String> = v
                            .fields
                            .iter()
                            .map(|(n, g)| format!("\"{n}\":{}", json_schema_gbnf_shaped(g)))
                            .collect();
                        format!(
                            "{{\"type\":\"object\",\"properties\":{{\"{}\":{{\"type\":\"object\",\"properties\":{{{}}}}}}}}}",
                            v.name,
                            props.join(",")
                        )
                    })
                    .collect();
                format!("{{\"oneOf\":[{}]}}", alts.join(","))
            }
        }
        Grammar::List { elem, lo, hi } => format!(
            "{{\"type\":\"array\",\"items\":{},\"minItems\":{lo},\"maxItems\":{hi}}}",
            json_schema_gbnf_shaped(elem)
        ),
    }
}

/// Walk a grammar and report, for each decode step (depth-first, the same order
/// the Mock generates), the set of permitted next-tokens. This is the canonical,
/// engine-independent notion of "what the grammar allowed here" that the
/// falsification test reasons over. A real engine maps these onto its own
/// vocabulary mask; the property under test (real forbids a token weakened
/// permits) is identical across representations.
pub fn permitted_tokens(grammar: &Grammar) -> Vec<Vec<String>> {
    let mut steps = Vec::new();
    collect_permitted(grammar, &mut steps);
    steps
}

fn collect_permitted(grammar: &Grammar, steps: &mut Vec<Vec<String>>) {
    match grammar {
        Grammar::Number { lo, hi } => {
            let mut toks = Vec::new();
            for n in *lo..=*hi {
                toks.push(n.to_string());
            }
            steps.push(toks);
        }
        Grammar::Bool => steps.push(vec!["true".into(), "false".into()]),
        Grammar::Text { .. } => {
            // Free text: the alphabet is the permitted set at the first step.
            // This is the "unconstrained" set a weakened type degrades to.
            let alphabet: Vec<String> = (b'a'..=b'z').map(|c| (c as char).to_string()).collect();
            steps.push(alphabet);
        }
        Grammar::Record(fields) => {
            for (_, g) in fields {
                collect_permitted(g, steps);
            }
        }
        Grammar::OneOf(variants) => {
            let names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
            steps.push(names);
            // The chosen variant's fields would follow, but for the litmus we
            // only need the closed top-level alternation to contrast against the
            // open weakened set.
        }
        Grammar::List { elem, lo, hi } => {
            // The list's masking lives in its CARDINALITY. We walk a canonical
            // maximal expansion: at each slot `i`, the decoder faces a
            // continue/stop decision —
            //   * `ITEM_i` (emit the (i+1)-th element) is permitted only while
            //     `i < hi`, so a slot beyond `hi` is forbidden; and
            //   * `STOP` is permitted only once `i >= lo`, so stopping early is
            //     forbidden.
            // Contrasting a strict `0..hi` against a weakened `0..hi'` (hi' > hi)
            // exposes `ITEM_hi` — the (hi+1)-th element — as a token the weakened
            // grammar permits but the strict grammar forbids: proof the bound
            // masked generation, not validated after. After each emittable slot
            // we recurse into `elem`, so widening the element type (e.g. variants
            // -> free text) is likewise caught at the element-content step.
            for i in 0..=*hi {
                let mut decision = Vec::new();
                if i < *hi {
                    decision.push(format!("ITEM_{i}"));
                }
                if i >= *lo {
                    decision.push("STOP".to_string());
                }
                steps.push(decision);
                if i < *hi {
                    collect_permitted(elem, steps);
                }
            }
        }
    }
}
