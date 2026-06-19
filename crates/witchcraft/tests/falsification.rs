//! The falsification test (change `add-inference-runtime`, Verification B).
//!
//! This is the canary that protects the AI-first thesis: it asserts that the
//! output type MASKS tokens DURING generation (a token the weakened grammar
//! permits is forbidden by the real grammar at some decode step), not that final
//! outputs merely differ. It runs against the Mock here (the deterministic
//! litmus oracle); the same harness runs against each real engine that claims
//! grammar-constrained decoding (behind the `llama`/`frontier` features).

use witchcraft::engine::mock::MockEngine;
use witchcraft::engine::{falsify, grammar_to_gbnf, grammar_to_json_schema, Engine};
use witchcraft::grammar::{Grammar, GrammarVariant};

/// A constrained output type: `Disposition`-like record with a refined urgency
/// and a closed variant action.
fn real_grammar() -> Grammar {
    Grammar::Record(vec![
        ("urgency".into(), Grammar::Number { lo: 0, hi: 10 }),
        (
            "action".into(),
            Grammar::OneOf(vec![
                GrammarVariant {
                    name: "Draft".into(),
                    fields: vec![],
                },
                GrammarVariant {
                    name: "Escalate".into(),
                    fields: vec![],
                },
                GrammarVariant {
                    name: "AskClarify".into(),
                    fields: vec![],
                },
            ]),
        ),
    ])
}

/// Deleting the type weakens generation to free text (the existing litmus knob).
/// `max_len` mirrors the glyph bound `compile(weaken=true)` produces; the litmus
/// contrast at step 0 is independent of the exact cap.
fn weakened_grammar() -> Grammar {
    Grammar::Text { max_len: 160 }
}

#[test]
fn mock_engine_masks_tokens_during_generation() {
    let mut engine = MockEngine::new(7, "TriageReasoner");
    let outcome = falsify(
        &mut engine,
        "TriageReasoner",
        "the customer is furious",
        &real_grammar(),
        &weakened_grammar(),
        7,
    );
    assert!(
        outcome.masked,
        "litmus must hold for the Mock engine: {}",
        outcome.reason
    );
    let witness = outcome.witness.expect("a masking witness is recorded");
    // The weakened (free-text) grammar permits a letter the real grammar (a
    // refined number at this step) forbids — proof the type masked the logits.
    assert!(
        !witness.forbidden_token.is_empty(),
        "witness names a forbidden token: {}",
        outcome.reason
    );
}

#[test]
fn falsification_fails_loudly_when_grammars_do_not_mask() {
    // Contrast: if the "real" and "weakened" grammars are identical, no masking
    // can be demonstrated and the harness must NOT report a false positive.
    let mut engine = MockEngine::new(1, "TriageReasoner");
    let g = weakened_grammar();
    let outcome = falsify(&mut engine, "TriageReasoner", "x", &g, &g, 1);
    assert!(
        !outcome.masked,
        "identical grammars cannot demonstrate masking, but harness claimed they did"
    );
    assert!(
        outcome.reason.contains("LITMUS FAILED"),
        "the harness reports the failure loudly: {}",
        outcome.reason
    );
}

#[test]
fn gbnf_closes_variants_and_bounds_numbers() {
    let g = real_grammar();
    let gbnf = grammar_to_gbnf(&g);
    // Closed variant alternation: exactly the declared names appear.
    assert!(gbnf.contains("Draft") && gbnf.contains("Escalate") && gbnf.contains("AskClarify"));
    // Refined number is a closed alternation of in-range integers (0..=10).
    assert!(gbnf.contains("\"10\""), "upper bound is reachable: {gbnf}");
    assert!(
        !gbnf.contains("\"11\""),
        "out-of-range value is unreachable: {gbnf}"
    );
}

#[test]
fn json_schema_encodes_range_and_strict_variant() {
    let g = real_grammar();
    let schema = grammar_to_json_schema(&g);
    assert!(schema.contains("\"minimum\":0") && schema.contains("\"maximum\":10"));
    // OpenAI strict: a variant is `anyOf` of discriminator-tagged closed objects
    // (NOT `oneOf`, which strict mode forbids), and every object is closed.
    assert!(
        schema.contains("\"anyOf\""),
        "variant action maps to anyOf: {schema}"
    );
    assert!(
        !schema.contains("\"oneOf\""),
        "oneOf is forbidden in OpenAI strict mode: {schema}"
    );
    assert!(
        schema.contains("\"tag\":{\"const\":\"Draft\"}"),
        "each branch carries a tag discriminator: {schema}"
    );
    assert!(
        schema.contains("\"additionalProperties\":false"),
        "every object is closed: {schema}"
    );
}

#[test]
fn mock_engine_is_grammar_constrained_and_litmus_safe() {
    let engine = MockEngine::new(0, "X");
    let desc = engine.describe();
    assert!(desc.grammar_constrained, "Mock enforces the grammar");
    assert_eq!(
        desc.litmus_safe,
        Some(true),
        "Mock is the deterministic litmus oracle"
    );
}

// ======================================================================
// add-constrained-list-generation: the LIST litmus (on-thesis).
// ======================================================================

/// A bounded list of closed variants — the dungeon `exits` shape.
fn directions() -> Grammar {
    Grammar::OneOf(vec![
        GrammarVariant {
            name: "North".into(),
            fields: vec![],
        },
        GrammarVariant {
            name: "South".into(),
            fields: vec![],
        },
        GrammarVariant {
            name: "East".into(),
            fields: vec![],
        },
        GrammarVariant {
            name: "West".into(),
            fields: vec![],
        },
    ])
}

fn exits_list(lo: u32, hi: u32) -> Grammar {
    Grammar::List {
        elem: Box::new(directions()),
        lo,
        hi,
    }
}

#[test]
fn mock_masks_list_cardinality_over_length_is_unreachable() {
    // The litmus for LIST COUNT: strict `0..4` vs weakened `0..10`, SAME element
    // type. The proof is NOT a final-string difference — it is that, at the
    // decode step where a 5th element would begin, the strict grammar FORBIDS
    // the token the weakened grammar permits. Masking during generation, witnessed.
    let mut engine = MockEngine::new(7, "DungeonMaster");
    let outcome = falsify(
        &mut engine,
        "DungeonMaster",
        "you are in a damp cellar",
        &exits_list(0, 4),
        &exits_list(0, 10),
        7,
    );
    assert!(
        outcome.masked,
        "list cardinality litmus must hold on the Mock: {}",
        outcome.reason
    );
    let witness = outcome.witness.expect("a cardinality masking witness");
    // The forbidden token is the (hi+1)-th element slot — the 5th exit. The
    // strict 0..4 grammar cannot open it; the weakened 0..10 grammar can.
    assert_eq!(
        witness.forbidden_token, "ITEM_4",
        "the witness must be the forbidden 5th element (slot 4): {}",
        outcome.reason
    );
}

#[test]
fn mock_masks_list_element_type_out_of_set_is_unreachable() {
    // The litmus for LIST ELEMENT TYPE: strict `list of 0..4 of one_of{N,S,E,W}`
    // vs a weakened list whose elements degrade to free text. At the first
    // element's content step the strict grammar forbids a letter the weakened
    // (free-text) element permits — an out-of-set "variant" is unreachable.
    let weak = Grammar::List {
        elem: Box::new(Grammar::Text { max_len: 160 }),
        lo: 0,
        hi: 4,
    };
    let mut engine = MockEngine::new(3, "DungeonMaster");
    let outcome = falsify(
        &mut engine,
        "DungeonMaster",
        "you are in a damp cellar",
        &exits_list(0, 4),
        &weak,
        3,
    );
    assert!(
        outcome.masked,
        "list element-type litmus must hold on the Mock: {}",
        outcome.reason
    );
    let witness = outcome.witness.expect("an element-type masking witness");
    // A lowercase letter the free-text element permits but the closed variant
    // set forbids.
    assert!(
        witness.forbidden_token.len() == 1
            && witness
                .forbidden_token
                .chars()
                .all(|c| c.is_ascii_lowercase()),
        "witness names an out-of-set element token: {:?}",
        witness.forbidden_token
    );
}

#[test]
fn list_gbnf_is_a_closed_length_disjunction_no_unbounded_repeat() {
    // The GBNF for a bounded list is a closed alternation of fixed lengths —
    // there is NO unbounded `*`/`+` repetition, so an over-length array is not a
    // member of the grammar (litmus-safe on llama.cpp by construction).
    let gbnf = grammar_to_gbnf(&exits_list(0, 2));
    assert!(
        !gbnf.contains('*') && !gbnf.contains('+'),
        "no unbounded repetition in the list GBNF: {gbnf}"
    );
    assert!(gbnf.contains("\"[]\""), "empty list is reachable: {gbnf}");
    // Exactly the declared directions appear as the element alternation.
    assert!(gbnf.contains("North") && gbnf.contains("West"));
}

#[test]
fn list_json_schema_bounds_item_count() {
    let schema = grammar_to_json_schema(&exits_list(0, 4));
    assert!(
        schema.contains("\"type\":\"array\""),
        "array schema: {schema}"
    );
    assert!(schema.contains("\"minItems\":0") && schema.contains("\"maxItems\":4"));
}

#[test]
fn glyph_field_is_a_quoted_json_string_in_gbnf() {
    // Regression: a `glyph` field nested in a record MUST be emitted as a quoted
    // JSON string, or the whole object is invalid JSON and `json_to_value` fails,
    // collapsing every field to its empty fallback (the dungeon-master bug).
    let g = Grammar::Record(vec![
        ("narration".into(), Grammar::Text { max_len: 8 }),
        ("danger".into(), Grammar::Number { lo: 0, hi: 3 }),
    ]);
    let gbnf = grammar_to_gbnf(&g);
    assert!(
        gbnf.contains("\"\\\"\""),
        "the glyph field value is wrapped in literal JSON quotes: {gbnf}"
    );
    // But a BARE (root) text output stays unquoted free prose — the weakened
    // "no type" form the litmus contrasts against (and `scalar_from_text` reads).
    let bare = grammar_to_gbnf(&Grammar::Text { max_len: 8 });
    assert!(
        !bare.contains("\\\""),
        "a root free-text output is unquoted: {bare}"
    );
}

/// The end-to-end round trip the real engines depend on: a record carrying a
/// glyph field, once generated as quoted JSON, parses back into POPULATED fields
/// (not the empty fallback). Built only when a real engine is enabled, since
/// `json_to_value` is part of that surface.
#[cfg(any(feature = "llama", feature = "frontier"))]
#[test]
fn record_with_glyph_round_trips_through_json() {
    use witchcraft::engine::json_to_value;
    let g = Grammar::Record(vec![
        ("narration".into(), Grammar::Text { max_len: 32 }),
        (
            "exits".into(),
            Grammar::List {
                elem: Box::new(directions()),
                lo: 0,
                hi: 4,
            },
        ),
        ("danger".into(), Grammar::Number { lo: 0, hi: 10 }),
    ]);
    let sample =
        "{\"narration\":\"You see a heavy door\",\"exits\":[\"North\",\"East\"],\"danger\":7}";
    let v = json_to_value(sample, &g).expect("a quoted-glyph record is valid JSON and parses");
    let rendered = v.display();
    assert!(
        rendered.contains("You see a heavy door"),
        "narration is populated, not empty: {rendered}"
    );
    assert!(
        rendered.contains("danger: 7"),
        "danger is populated: {rendered}"
    );
    assert!(
        rendered.contains("North") && rendered.contains("East"),
        "exits are populated: {rendered}"
    );
}

/// `json_to_value` parses BOTH variant wire forms: the frontier OpenAI-strict
/// discriminator-tagged object (`{"tag":"Slip","detail":…}`) and the llama GBNF
/// forms (a bare `"Name"` string, a nested `{"Name":{…}}` object). The frontier
/// schema fix changed the schema, not the parser's tolerance — both must work.
#[cfg(any(feature = "llama", feature = "frontier"))]
#[test]
fn variant_parses_from_both_tagged_and_nested_forms() {
    use witchcraft::engine::json_to_value;
    let tell = Grammar::OneOf(vec![
        GrammarVariant {
            name: "Nothing".into(),
            fields: vec![],
        },
        GrammarVariant {
            name: "Slip".into(),
            fields: vec![("detail".into(), Grammar::Text { max_len: 160 })],
        },
    ]);

    // Frontier OpenAI-strict tagged form: payload fields are siblings of `tag`.
    let tagged = json_to_value("{\"tag\":\"Slip\",\"detail\":\"the safe\"}", &tell)
        .expect("tagged payload variant parses");
    assert!(
        tagged.display().contains("Slip") && tagged.display().contains("the safe"),
        "tagged variant carries its payload: {}",
        tagged.display()
    );
    let tagged_bare =
        json_to_value("{\"tag\":\"Nothing\"}", &tell).expect("tagged bare variant parses");
    assert!(tagged_bare.display().contains("Nothing"));

    // llama GBNF forms still parse (the parser stayed permissive).
    let nested = json_to_value("{\"Slip\":{\"detail\":\"the safe\"}}", &tell)
        .expect("nested payload variant parses");
    assert!(nested.display().contains("Slip") && nested.display().contains("the safe"));
    let string_bare = json_to_value("\"Nothing\"", &tell).expect("bare variant as a string parses");
    assert!(string_bare.display().contains("Nothing"));
}

/// THE MAKE-OR-BREAK: run the falsification test against REAL llama.cpp weights,
/// using its real GBNF sampler — not the Mock. The model is named only in the
/// manifest (the contract); this test reads the GGUF path from `WITCHCRAFT_GGUF`
/// and resolves the engine through `Manifest::resolve`, exactly as a deployment
/// would. The headline assertion is that the type MASKED a token during
/// generation on a real tokenizer: a concrete forbidden token at a concrete
/// decode step that the weakened grammar permitted but the real grammar's
/// sampler drove to -inf. Run with:
///
///   WITCHCRAFT_GGUF=$PWD/models/<model>.gguf \
///     cargo test --features llama real_llama -- --nocapture
///
/// §8 honesty: this proves the type masks SHAPE, never that the output is good.
#[cfg(feature = "llama")]
#[test]
fn real_llama_masks_tokens_during_generation() {
    use witchcraft::engine::falsify;
    use witchcraft::manifest::Manifest;

    let gguf = match std::env::var("WITCHCRAFT_GGUF") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!(
                "SKIP real_llama_masks_tokens_during_generation: set WITCHCRAFT_GGUF to a \
                 local GGUF path to run the make-or-break litmus against real weights."
            );
            return;
        }
    };

    // The model is named ONLY here (a synthetic manifest), per the contract.
    let manifest_src = format!(
        "[need.TriageReasoner]\nengine = \"local-llm\"\nlocality = \"local\"\n\n\
         [engine.local-llm]\nkind = \"llama\"\ngguf = \"{gguf}\"\n"
    );
    let manifest = Manifest::parse(&manifest_src).expect("manifest parses");
    let policy = witchcraft::engine::Policy::default();
    let mut engine = manifest
        .resolve("TriageReasoner", &policy, 7)
        .unwrap_or_else(|e| panic!("{}", e.message()));

    assert_eq!(
        engine.describe().backend_id,
        "llama",
        "the falsification must run against the real llama engine, not the Mock"
    );

    let outcome = falsify(
        engine.as_mut(),
        "TriageReasoner",
        "the customer is furious about a double charge",
        &real_grammar(),
        &weakened_grammar(),
        7,
    );

    // For a fuller, honest picture, also show what the real model actually
    // GENERATED under each grammar (token-by-token through the real sampler):
    // the typed grammar yields an in-type structured value by construction; the
    // weakened grammar is free to wander out of it.
    use witchcraft::engine::{InferRequest, Policy};
    let p = Policy::default();
    let real_out = engine.infer(&InferRequest {
        intent_id: "TriageReasoner",
        input: "the customer is furious about a double charge",
        grammar: &real_grammar(),
        policy: &p,
        seed: 7,
    });
    let weak_out = engine.infer(&InferRequest {
        intent_id: "TriageReasoner",
        input: "the customer is furious about a double charge",
        grammar: &weakened_grammar(),
        policy: &p,
        seed: 7,
    });

    eprintln!("=== REAL-SAMPLER MASKING WITNESS (llama.cpp / GBNF) ===");
    eprintln!("backend     : {}", outcome.backend_id);
    eprintln!("masked      : {}", outcome.masked);
    eprintln!("reason      : {}", outcome.reason);
    eprintln!(
        "real-type generation (in-type by construction): {:?}",
        real_out.value
    );
    eprintln!(
        "weakened generation  (free to leave the type) : {:?}",
        weak_out.value
    );
    // The confidence is now read from the sampler (the geometric mean of the
    // masked probability the model assigned each chosen token), not a 1.0
    // placeholder. It is a genuine probability in (0, 1].
    eprintln!(
        "real-type confidence (sampler chosen-token prob, geom-mean): {}",
        real_out.confidence
    );
    eprintln!(
        "weakened confidence  (sampler chosen-token prob, geom-mean): {}",
        weak_out.confidence
    );
    assert!(
        real_out.confidence > 0.0 && real_out.confidence <= 1.0,
        "confidence must be a real probability in (0, 1], got {}",
        real_out.confidence
    );
    if let Some(w) = &outcome.witness {
        eprintln!("witness.step           : {}", w.step);
        eprintln!("witness.forbidden_token: {:?}", w.forbidden_token);
        eprintln!(
            "interpretation         : at decode step {}, the real type's GBNF sampler drove \
             token {:?} to -inf; the weakened (free-text) grammar permitted it. The type masked \
             generation on real weights.",
            w.step, w.forbidden_token
        );
    }
    eprintln!("=======================================================");

    assert!(
        outcome.masked,
        "LITMUS FAILED against real llama.cpp: {}",
        outcome.reason
    );
    let witness = outcome
        .witness
        .expect("a real-sampler masking witness is recorded");
    assert!(
        !witness.forbidden_token.is_empty(),
        "the witness must name a concrete forbidden token"
    );
}

/// THE LIST MAKE-OR-BREAK against REAL llama.cpp weights. Two contrasts run
/// through the real GBNF sampler:
///
///   1. STRUCTURE (the hard assertion): the list type `list of 0..4 of
///      one_of {N,S,E,W}` vs a deleted type (free text). The list grammar forces
///      the first token to open a JSON array (`[`), forbidding the free-text
///      tokens the weakened grammar permits — proof the list type masks
///      generation on real weights. This is reliable across tokenizers (it only
///      probes the first decode step, exactly like the scalar litmus).
///
///   2. COUNT BOUND (a best-effort bonus, reported not asserted): strict `0..4`
///      vs weakened `0..8`. After four legal elements the strict grammar should
///      drive `,` to -inf (the fifth exit unreachable). This requires building a
///      grammar-aligned token prefix, which is tokenizer-dependent; when a model
///      tokenizes the JSON prefix in a way the probe cannot align, the boundary
///      is reported INCONCLUSIVE rather than faked. The COUNT-bound litmus is
///      proven deterministically and unconditionally on the Mock engine
///      (`mock_masks_list_cardinality_over_length_is_unreachable`).
///
///   WITCHCRAFT_GGUF=$PWD/models/<model>.gguf \
///     cargo test --features llama real_llama_masks_list -- --nocapture
///
/// §8 honesty: this proves the type masks SHAPE/COUNT, never that the chosen
/// exits are good gameplay.
#[cfg(feature = "llama")]
#[test]
fn real_llama_masks_list_cardinality() {
    use witchcraft::engine::falsify;
    use witchcraft::manifest::Manifest;

    let gguf = match std::env::var("WITCHCRAFT_GGUF") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!(
                "SKIP real_llama_masks_list_cardinality: set WITCHCRAFT_GGUF to a local GGUF \
                 path to run the list litmus against real weights."
            );
            return;
        }
    };

    let manifest_src = format!(
        "[need.DungeonMaster]\nengine = \"local-llm\"\nlocality = \"local\"\n\n\
         [engine.local-llm]\nkind = \"llama\"\ngguf = \"{gguf}\"\n"
    );
    let manifest = Manifest::parse(&manifest_src).expect("manifest parses");
    let policy = witchcraft::engine::Policy::default();
    let mut engine = manifest
        .resolve("DungeonMaster", &policy, 7)
        .unwrap_or_else(|e| panic!("{}", e.message()));
    assert_eq!(engine.describe().backend_id, "llama");

    // (1) STRUCTURE witness — the reliable, asserted proof that the list type
    // masks generation on real weights.
    let structure = falsify(
        engine.as_mut(),
        "DungeonMaster",
        "you are in a damp cellar with passages leading away",
        &exits_list(0, 4),
        &Grammar::Text { max_len: 160 },
        7,
    );
    eprintln!("=== REAL-SAMPLER LIST-STRUCTURE WITNESS (llama.cpp / GBNF) ===");
    eprintln!("masked  : {}", structure.masked);
    eprintln!("reason  : {}", structure.reason);
    if let Some(w) = &structure.witness {
        eprintln!(
            "witness : at step {} the list grammar forbade {:?} (a token free text permits) — \
             the list type masked generation on real weights.",
            w.step, w.forbidden_token
        );
    }

    // (2) COUNT-BOUND bonus — reported honestly, asserted only if obtainable on
    // this tokenizer (the deterministic proof lives on the Mock).
    let cardinality = falsify(
        engine.as_mut(),
        "DungeonMaster",
        "you are in a damp cellar with passages leading away",
        &exits_list(0, 4),
        &exits_list(0, 8),
        7,
    );
    eprintln!("--- count-bound bonus (strict 0..4 vs weakened 0..8) ---");
    if cardinality.masked {
        let w = cardinality.witness.as_ref().expect("witness when masked");
        eprintln!(
            "witness : at step {} the strict 0..4 grammar forbade {:?} (a continuation the \
             weakened 0..8 permitted) — the 5th exit is unreachable on real weights.",
            w.step, w.forbidden_token
        );
    } else {
        eprintln!(
            "INCONCLUSIVE on this tokenizer: the count-bound boundary probe could not align a \
             grammar-exact JSON prefix to this model's tokens. NOT a litmus failure — the \
             count-bound litmus is proven deterministically on the Mock engine."
        );
    }
    eprintln!("==============================================================");

    assert!(
        structure.masked,
        "LIST LITMUS FAILED against real llama.cpp: {}",
        structure.reason
    );
    let witness = structure
        .witness
        .expect("a real-sampler list-structure masking witness is recorded");
    assert!(
        !witness.forbidden_token.is_empty(),
        "the witness must name a concrete forbidden token"
    );
}
