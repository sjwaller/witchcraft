//! Backend tests (group 3): the Cranelift-compiled host language must produce
//! the same observable output as the interpreter for the same program and seed
//! (the D6 equivalence requirement), and loop-local heap values must be
//! reclaimed during execution (group 2.3/2.4 in compiled form).

use witchcraft::{lower_source, run_source, RunConfig};
use witchcraft_codegen::{run, run_capture};

/// Compile + run, returning captured stdout.
fn compiled(src: &str, seed: u64) -> String {
    let ir = lower_source(src).unwrap_or_else(|ds| {
        panic!(
            "lowering failed: {}",
            ds.iter().map(|d| d.render()).collect::<Vec<_>>().join("; ")
        )
    });
    run_capture(&ir, seed).expect("compiled run")
}

/// Interpret, returning stdout.
fn interpreted(src: &str, seed: u64) -> String {
    run_source(
        src,
        RunConfig {
            seed,
            ..Default::default()
        },
    )
    .unwrap_or_else(|ds| {
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
