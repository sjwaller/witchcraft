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
