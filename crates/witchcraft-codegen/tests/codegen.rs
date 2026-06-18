//! Backend tests (group 3): the Cranelift-compiled host language must produce
//! the same observable output as the interpreter for the same program and seed
//! (the D6 equivalence requirement), and loop-local heap values must be
//! reclaimed during execution (group 2.3/2.4 in compiled form).

use witchcraft::{lower_source, lower_source_weaken, run_source, RunConfig};
use witchcraft_codegen::{run, run_capture, run_capture_with, RunOptions};

/// Compile + run, returning captured stdout.
fn compiled(src: &str, seed: u64) -> String {
    run_capture(&lower(src), seed).expect("compiled run")
}

fn lower(src: &str) -> witchcraft::ir::Program {
    lower_source(src).unwrap_or_else(|ds| {
        panic!(
            "lowering failed: {}",
            ds.iter().map(|d| d.render()).collect::<Vec<_>>().join("; ")
        )
    })
}

/// Interpret, returning stdout.
fn interpreted(src: &str, seed: u64) -> String {
    interpreted_with(
        src,
        RunConfig {
            seed,
            ..Default::default()
        },
    )
}

fn interpreted_with(src: &str, config: RunConfig) -> String {
    run_source(src, config).unwrap_or_else(|ds| {
        panic!(
            "interpret failed: {}",
            ds.iter().map(|d| d.render()).collect::<Vec<_>>().join("; ")
        )
    })
}

fn assert_equivalent(src: &str) {
    let a = compiled(src, 0);
    let b = interpreted(src, 0);
    assert_eq!(a, b, "compiled and interpreted output differ\nsrc:\n{src}");
}

#[test]
fn arithmetic_and_functions() {
    assert_equivalent("fn add(a, b) { a + b }\nprint add(2, 3)");
    assert_equivalent("print 2 + 3 * 4 - 1");
    assert_equivalent("print 10 / 4");
}

#[test]
fn comparisons_and_equality_and_booleans() {
    assert_equivalent("print 1 < 2\nprint 3 <= 3\nprint 5 > 9\nprint 4 >= 4");
    assert_equivalent("print 2 == 2\nprint 2 == 3\nprint 1 != 2");
    assert_equivalent("print true and false\nprint true or false\nprint not true");
}

#[test]
fn glyph_interpolation() {
    assert_equivalent("let who = \"witch\"\nprint \"hi ${who}, ${1 + 1} times\"");
}

#[test]
fn control_flow() {
    assert_equivalent("var n = 0\nwhile n < 3 { print n n = n + 1 }");
    assert_equivalent("if 2 + 2 == 4 { print \"ok\" } else { print \"no\" }");
}

#[test]
fn host_example_matches_interpreter() {
    let src = include_str!("../../../examples/host.witch");
    assert_equivalent(src);
}

#[test]
fn host_example_runs_to_stdout_without_capture() {
    // Exercise the real `run` path (prints to stdout) to make sure it executes.
    let src = include_str!("../../../examples/host.witch");
    let ir = lower_source(src).expect("lower");
    run(&ir, 0).expect("run");
}

// ---------- group 4: divine / oracle / enact in compiled form ----------

const ACTION_TYPES: &str = "\
type Action = one_of {
    Draft(reply: glyph),
    Escalate,
    AskClarify(question: glyph),
}
type Disposition = { urgency: spark in 0..10, action: Action }
";

#[test]
fn triage_example_compiles_to_the_interpreter_golden() {
    // The §6.3 worked example: divine an inferred Disposition, discharge it, and
    // enact over the variants — with provenance threaded into the Draft arm. The
    // compiled artifact must reproduce the interpreter's deterministic golden.
    let src = include_str!("../../../examples/triage.witch");
    let expected = "\
urgency: 8
drafted reply: fcrlysheyyil
provenance: oracle=triage model=mock-triage-v1 seed=1
";
    assert_eq!(compiled(src, 1), expected);
    assert_eq!(compiled(src, 1), interpreted(src, 1));
}

#[test]
fn divine_field_access_matches_interpreter() {
    let src = format!(
        "{ACTION_TYPES}
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o with confidence >= 0.0 fallback \"f\"
print d.urgency
"
    );
    for seed in [0u64, 1, 7, 42] {
        assert_eq!(compiled(&src, seed), interpreted(&src, seed), "seed {seed}");
    }
}

#[test]
fn compiled_litmus_deleting_the_type_changes_generation() {
    // Same program + seed; once with the output type in force, once with it
    // structurally removed (weakened to free text). The compiled artifacts must
    // differ — the type is part of the computation, not documentation (§6.3).
    let src = "\
type Rating = spark in 0..5
oracle o = summon \"m\"
divine r: Rating from (\"x\") using o with confidence >= 0.0 fallback 0
print r
";
    let typed = run_capture(&lower(src), 3).expect("typed run");
    let untyped =
        run_capture(&lower_source_weaken(src, true).expect("weaken lower"), 3).expect("weak run");
    assert_ne!(
        typed, untyped,
        "deleting the type must change generation (litmus)"
    );
    let n: i64 = typed.trim().parse().expect("typed output is a spark");
    assert!((0..=5).contains(&n), "constrained output {n} out of range");
}

#[test]
fn compiled_fault_injection_keeps_low_confidence_out_of_enact() {
    // Forcing the discharge to see low confidence fires the fallback and unwinds,
    // so no enact arm and no trailing statement run — the §6.2 guarantee, in
    // compiled form. Forcing high confidence completes the run.
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
    let ir = lower(&src);

    let injected = run_capture_with(
        &ir,
        RunOptions {
            seed: 1,
            force_confidence: Some(0.0),
        },
    )
    .expect("injected run");
    assert_eq!(injected, "start\n");
    assert!(!injected.contains("drafted"));
    assert!(!injected.contains("escalated"));

    let healthy = run_capture_with(
        &ir,
        RunOptions {
            seed: 1,
            force_confidence: Some(1.0),
        },
    )
    .expect("healthy run");
    assert!(healthy.contains("end"));

    // And both match the interpreter under the same fault injection.
    assert_eq!(
        injected,
        interpreted_with(
            &src,
            RunConfig {
                seed: 1,
                force_confidence: Some(0.0),
                ..Default::default()
            }
        )
    );
    assert_eq!(
        healthy,
        interpreted_with(
            &src,
            RunConfig {
                seed: 1,
                force_confidence: Some(1.0),
                ..Default::default()
            }
        )
    );
}

#[test]
fn loop_local_heap_is_reclaimed_in_compiled_code() {
    // A glyph is allocated each iteration (via interpolation) and printed. The
    // emitted reference-counting must reclaim it each iteration so the live heap
    // count returns to its baseline after the loop.
    let src = "var n = 0\nwhile n < 5000 { print \"n=${n}\" n = n + 1 }";
    let ir = lower_source(src).expect("lower");
    let before = witchcraft_runtime::live_objects();
    let out = run_capture(&ir, 0).expect("run");
    let after = witchcraft_runtime::live_objects();
    assert_eq!(out.lines().count(), 5000);
    assert_eq!(
        after, before,
        "compiled loop leaked heap values (before={before}, after={after})"
    );
}
