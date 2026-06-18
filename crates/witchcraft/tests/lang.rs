//! Acceptance tests for the v0.1 thesis. These are the tests that would fail if
//! Witchcraft were "sugar over an SDK" rather than AI-native:
//!
//!   * litmus (§6.3): deleting the output type changes what is generated.
//!   * fault injection (§6.2): a low-confidence value cannot flow into `enact`.
//!   * discharge / refinement / exhaustiveness: structural errors are caught at
//!     check time, not runtime.

use witchcraft::{check_source, run_source, RunConfig};

const ACTION_TYPES: &str = "\
type Action = one_of {
    Draft(reply: glyph),
    Escalate,
    AskClarify(question: glyph),
}
type Disposition = { urgency: spark in 0..10, action: Action }
";

fn run(src: &str, config: RunConfig) -> String {
    run_source(src, config).unwrap_or_else(|ds| {
        panic!(
            "expected program to run, got: {}",
            ds.iter().map(|d| d.render()).collect::<Vec<_>>().join("; ")
        )
    })
}

fn check_err(src: &str) -> String {
    match check_source(src) {
        Ok(()) => panic!("expected a compile error, but the program checked clean"),
        Err(ds) => ds
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

// ---------- the host language ----------

#[test]
fn host_arithmetic_and_control_flow() {
    let src = "\
fn add(a, b) { a + b }
var n = 0
while n < 3 { print n n = n + 1 }
print add(2, 3)
";
    assert_eq!(run(src, RunConfig::default()), "0\n1\n2\n5\n");
}

#[test]
fn glyph_interpolation() {
    let src = "let who = \"witch\" print \"hi ${who}\"";
    assert_eq!(run(src, RunConfig::default()), "hi witch\n");
}

#[test]
fn let_is_immutable_var_is_not() {
    let immutable = "let x = 1 x = 2";
    let err = run_source(immutable, RunConfig::default()).unwrap_err();
    assert!(err[0].render().contains("cannot reassign"));

    let mutable = "var x = 1 x = 2 print x";
    assert_eq!(run(mutable, RunConfig::default()), "2\n");
}

#[test]
fn division_by_zero_is_a_runtime_error() {
    let err = run_source("print 1 / 0", RunConfig::default()).unwrap_err();
    assert!(err[0].render().contains("division by zero"));
}

#[test]
fn functions_do_not_leak_caller_locals() {
    // `local` lives inside `g`; `f` must not see it through the call.
    let src = "fn f() { local } fn g() { let local = 5 f() } print g()";
    let err = run_source(src, RunConfig::default()).unwrap_err();
    assert!(err[0].render().contains("undefined name `local`"));
}

#[test]
fn missing_type_is_a_compile_error() {
    let err = check_err("let x: Nope = 1");
    assert!(err.contains("unknown type"));
}

// ---------- determinism ----------

#[test]
fn same_seed_same_output() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o with confidence >= 0.0 fallback \"f\"
print d.urgency
"
    );
    let a = run(
        &src,
        RunConfig {
            seed: 42,
            ..Default::default()
        },
    );
    let b = run(
        &src,
        RunConfig {
            seed: 42,
            ..Default::default()
        },
    );
    assert_eq!(a, b);
}

// ---------- the litmus test (§6.3) ----------

#[test]
fn litmus_deleting_the_type_changes_the_computation() {
    // Same program, same seed. Once with the output type in force; once with the
    // type structurally removed (weakened to free text). If the type were mere
    // documentation, these would be identical.
    let src = "\
type Rating = spark in 0..5
oracle o = summon \"m\"
divine r: Rating from (\"x\") using o with confidence >= 0.0 fallback 0
print r
";
    let typed = run(
        src,
        RunConfig {
            seed: 3,
            ..Default::default()
        },
    );
    let untyped = run(
        src,
        RunConfig {
            seed: 3,
            weaken_divine: true,
            ..Default::default()
        },
    );
    assert_ne!(
        typed, untyped,
        "deleting the type must change generation (litmus)"
    );
    // With the type, the result is a number within the refinement.
    let n: i64 = typed.trim().parse().expect("typed output is a spark");
    assert!(
        (0..=5).contains(&n),
        "constrained output {} out of range",
        n
    );
}

// ---------- fault injection (§6.2) ----------

#[test]
fn low_confidence_value_never_reaches_enact() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
print \"start\"
divine d: Disposition from (\"t\") using o with confidence >= 0.8 fallback \"fb\"
enact d.action {{
    Draft(reply) => {{ print \"drafted\" }}
    Escalate => {{ print \"escalated\" }}
    AskClarify(question) => {{ print \"asked\" }}
}}
print \"end\"
"
    );

    // Force the discharge to see low confidence: the fallback fires and unwinds,
    // so no enact arm and no trailing statement runs.
    let injected = run(
        &src,
        RunConfig {
            seed: 1,
            force_confidence: Some(0.0),
            ..Default::default()
        },
    );
    assert_eq!(injected, "start\n");
    assert!(!injected.contains("drafted"));
    assert!(!injected.contains("escalated"));

    // With confidence forced high, an action is enacted and execution completes.
    let healthy = run(
        &src,
        RunConfig {
            seed: 1,
            force_confidence: Some(1.0),
            ..Default::default()
        },
    );
    assert!(healthy.contains("end"));
    assert!(
        healthy.contains("drafted") || healthy.contains("escalated") || healthy.contains("asked")
    );
}

// ---------- structural compile errors (negative tests) ----------

#[test]
fn refinement_out_of_range_is_a_compile_error() {
    let src = "type R = spark in 0..10\nlet x: R = 11";
    assert!(check_err(src).contains("outside the refinement bound"));
}

#[test]
fn undischarged_inferred_value_cannot_be_used() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o
enact d.action {{
    Draft(reply) => {{}}
    Escalate => {{}}
    AskClarify(question) => {{}}
}}
"
    );
    assert!(check_err(&src).contains("discharged"));
}

#[test]
fn non_exhaustive_enact_is_a_compile_error() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o with confidence >= 0.0 fallback \"f\"
enact d.action {{
    Draft(reply) => {{}}
    Escalate => {{}}
}}
"
    );
    assert!(check_err(&src).contains("non-exhaustive"));
}

#[test]
fn unknown_variant_in_enact_is_a_compile_error() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o with confidence >= 0.0 fallback \"f\"
enact d.action {{
    Draft(reply) => {{}}
    Escalate => {{}}
    AskClarify(question) => {{}}
    Bogus => {{}}
}}
"
    );
    assert!(check_err(&src).contains("unknown variant"));
}

// ---------- capabilities: surface syntax (compile-time only) ----------

#[test]
fn parses_fn_requires_and_grant_region() {
    use witchcraft::ast::{Item, Stmt};
    use witchcraft::parser::parse;
    let src = "\
fn escalate() requires permit(escalate) { print \"escalated\" }
with grant permit(escalate) {
    escalate()
}
";
    let prog = parse(src).expect("should parse");
    let f = prog
        .items
        .iter()
        .find_map(|i| match i {
            Item::Fn(f) => Some(f),
            _ => None,
        })
        .expect("a fn item");
    assert_eq!(f.requires.len(), 1);
    assert_eq!(f.requires[0].display(), "permit(escalate)");
    assert!(prog
        .items
        .iter()
        .any(|i| matches!(i, Item::Stmt(Stmt::Grant { .. }))));
    // Capabilities are erased at run time: the granted region's body runs.
    assert_eq!(run(src, RunConfig::default()), "escalated\n");
}

#[test]
fn parses_multiple_and_parameterised_capabilities() {
    use witchcraft::ast::Item;
    use witchcraft::parser::parse;
    let src = "\
fn act() requires permit(escalate), scope(tenant) { print \"ok\" }
with grant permit(escalate), scope(tenant) {
    act()
}
";
    let prog = parse(src).expect("should parse");
    let f = prog
        .items
        .iter()
        .find_map(|i| match i {
            Item::Fn(f) => Some(f),
            _ => None,
        })
        .unwrap();
    let names: Vec<_> = f.requires.iter().map(|c| c.display()).collect();
    assert_eq!(names, vec!["permit(escalate)", "scope(tenant)"]);
    assert_eq!(run(src, RunConfig::default()), "ok\n");
}

#[test]
fn capability_without_a_parameter_parses() {
    use witchcraft::ast::Item;
    use witchcraft::parser::parse;
    let prog = parse("fn a() requires audit { }").expect("bare capability parses");
    let f = match &prog.items[0] {
        Item::Fn(f) => f,
        _ => unreachable!(),
    };
    assert_eq!(f.requires[0].kind, "audit");
    assert_eq!(f.requires[0].param, None);
}

#[test]
fn malformed_capability_is_a_parse_error() {
    use witchcraft::parser::parse;
    // Unclosed capability parameter.
    assert!(parse("fn a() requires permit( { }").is_err());
    // A grant region with no capability named.
    assert!(parse("with grant { }").is_err());
}

// ---------- capabilities: checking (compile-time guarantee) ----------

#[test]
fn missing_capability_is_a_compile_error() {
    let src = "\
fn escalate() requires permit(escalate) { print \"e\" }
escalate()
";
    let err = check_err(src);
    assert!(
        err.contains("permit(escalate)"),
        "names the capability: {err}"
    );
    assert!(err.contains("escalate"), "names the operation: {err}");
}

#[test]
fn granted_capability_passes_the_check() {
    let src = "\
fn escalate() requires permit(escalate) { print \"e\" }
with grant permit(escalate) { escalate() }
";
    assert!(
        check_source(src).is_ok(),
        "granting the capability should pass"
    );
}

#[test]
fn caller_without_capability_fails_transitively() {
    // `b` neither grants nor re-declares the capability `a` requires.
    let src = "\
fn a() requires permit(escalate) { print \"a\" }
fn b() { a() }
with grant permit(escalate) { b() }
";
    assert!(check_err(src).contains("permit(escalate)"));
}

#[test]
fn caller_that_redeclares_passes_transitively() {
    let src = "\
fn a() requires permit(escalate) { print \"a\" }
fn b() requires permit(escalate) { a() }
with grant permit(escalate) { b() }
";
    assert!(check_source(src).is_ok());
}

#[test]
fn distinct_capability_parameters_are_distinct() {
    // Granting scope(tenant) does not satisfy a requirement for scope(user).
    let src = "\
fn read_user() requires scope(user) { print \"u\" }
with grant scope(tenant) { read_user() }
";
    assert!(check_err(src).contains("scope(user)"));
}

#[test]
fn grant_does_not_leak_past_its_region() {
    // The call inside the region is fine; the one after it is not.
    let src = "\
fn escalate() requires permit(escalate) { print \"e\" }
with grant permit(escalate) { escalate() }
escalate()
";
    assert!(check_err(src).contains("permit(escalate)"));
}

// ---------- golden output ----------

#[test]
fn triage_example_is_deterministic_golden() {
    let src = include_str!("../../../examples/triage.witch");
    let out = run(
        src,
        RunConfig {
            seed: 1,
            ..Default::default()
        },
    );
    let expected = "\
urgency: 8
drafted reply: fcrlysheyyil
provenance: intent=mock-triage-v1 model=mock-triage-v1 version=mock backend=mock seed=1 sampling=deterministic
";
    assert_eq!(out, expected);
}

#[test]
fn host_example_golden() {
    let src = include_str!("../../../examples/host.witch");
    let out = run(src, RunConfig::default());
    let expected = "\
Greetings, witch!
n = 0
n = 1
n = 2
arithmetic holds
";
    assert_eq!(out, expected);
}
