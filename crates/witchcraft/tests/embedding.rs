//! Typed-embedding acceptance tests (§5.3): an embedding carries its space in its
//! type, so cross-space comparison is a compile-time error, while same-space
//! similarity/nearest are deterministic and offline.

use witchcraft::{check_source, run_source, RunConfig};

fn run(src: &str) -> String {
    run_source(src, RunConfig::default()).unwrap_or_else(|ds| {
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

#[test]
fn embed_yields_the_oracles_space() {
    let src = "\
oracle triage = summon \"support-reasoner-v3\"
let e = triage.embed(\"payment failed\")
print e
";
    assert_eq!(run(src), "<embedding@support-reasoner-v3>\n");
}

#[test]
fn same_space_similarity_is_deterministic_and_meaningful() {
    // Identical text embeds to the same vector (similarity ~1); unrelated text is
    // less similar. The comparison is in-language so it is robust to float noise.
    let src = "\
oracle m = summon \"space-A\"
let p1 = m.embed(\"payment failed\")
let p2 = m.embed(\"payment failed\")
let q = m.embed(\"the weather is nice today\")
print similarity(p1, p2) >= similarity(p1, q)
";
    assert_eq!(run(src), "true\n");
    // Deterministic across runs.
    assert_eq!(run(src), run(src));
}

#[test]
fn nearest_returns_k_within_a_space_deterministically() {
    let src = "\
oracle m = summon \"space-A\"
let q = m.embed(\"payment failed\")
let c1 = m.embed(\"payment failed\")
let c2 = m.embed(\"unrelated topic entirely\")
let c3 = m.embed(\"another different subject\")
print nearest(q, [c1, c2, c3], 2)
";
    let out = run(src);
    // Two results, both in the same space.
    assert_eq!(out, "[<embedding@space-A>, <embedding@space-A>]\n");
    assert_eq!(run(src), run(src));
}

#[test]
fn cross_space_similarity_is_a_compile_error() {
    let src = "\
oracle a = summon \"space-A\"
oracle b = summon \"space-B\"
let ea = a.embed(\"x\")
let eb = b.embed(\"y\")
print similarity(ea, eb)
";
    let err = check_err(src);
    assert!(err.contains("space-A"), "names space A: {err}");
    assert!(err.contains("space-B"), "names space B: {err}");
}

#[test]
fn cross_space_nearest_is_a_compile_error() {
    let src = "\
oracle a = summon \"space-A\"
oracle b = summon \"space-B\"
let q = a.embed(\"x\")
let c = b.embed(\"y\")
print nearest(q, [c], 1)
";
    let err = check_err(src);
    assert!(err.contains("space-A") && err.contains("space-B"), "{err}");
}

#[test]
fn no_implicit_bridge_between_spaces() {
    // An embedding of space A cannot stand in where space B is required.
    let src = "\
oracle a = summon \"A\"
oracle b = summon \"B\"
let ea = a.embed(\"x\")
let eb = b.embed(\"y\")
let same = similarity(ea, eb)
print same
";
    assert!(check_source(src).is_err());
}
