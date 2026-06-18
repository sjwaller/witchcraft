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
fn weakened_grammar() -> Grammar {
    Grammar::Text { max_len: 16 }
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
fn json_schema_encodes_range_and_enum() {
    let g = real_grammar();
    let schema = grammar_to_json_schema(&g);
    assert!(schema.contains("\"minimum\":0") && schema.contains("\"maximum\":10"));
    assert!(
        schema.contains("\"enum\""),
        "variant action maps to an enum: {schema}"
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
