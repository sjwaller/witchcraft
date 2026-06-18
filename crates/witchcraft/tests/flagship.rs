//! Triage flagship acceptance tests (§6.3): the four structural guarantees
//! composed in one program, plus the four "will not compile" contrasts, the
//! type-as-litmus check, and low-confidence fault injection. Composition adds no
//! new language feature — every construct is from bootstrap or a primitive change.

use witchcraft::{check_source, run_source, RunConfig};

const FLAGSHIP: &str = include_str!("../../../examples/triage_flagship.witch");

const HEADER: &str = "\
type Action = one_of { Draft(reply: glyph), Escalate, AskClarify(question: glyph) }
type Disposition = { urgency: spark in 0..10, action: Action }
oracle triage = summon \"mock-triage-v1\"
";

fn run_cfg(src: &str, cfg: RunConfig) -> String {
    run_source(src, cfg).unwrap_or_else(|ds| {
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
fn flagship_runs_end_to_end_with_provenance() {
    let out = run_cfg(FLAGSHIP, RunConfig::default());
    assert!(
        out.contains("recalled scoped history"),
        "memory retrieval ran: {out}"
    );
    assert!(
        out.contains("provenance: oracle=triage model=mock-triage-v1"),
        "enacted action carries provenance: {out}"
    );
}

#[test]
fn flagship_is_reproducible_under_a_fixed_seed() {
    let cfg = RunConfig {
        seed: 7,
        ..Default::default()
    };
    assert_eq!(run_cfg(FLAGSHIP, cfg.clone()), run_cfg(FLAGSHIP, cfg));
}

// --- The four "will not compile" contrasts, under composition ---

#[test]
fn undischarged_divine_will_not_compile() {
    let src = format!(
        "{HEADER}
divine decision: Disposition from (\"x\") using triage
print decision.urgency
"
    );
    assert!(
        check_err(&src).to_lowercase().contains("discharge")
            || check_err(&src).contains("inferred"),
        "discharge error: {}",
        check_err(&src)
    );
}

#[test]
fn unscoped_memory_read_will_not_compile() {
    let src = "\
memory tickets { scope tenant }
print tickets.recent(5)
";
    let err = check_err(src);
    assert!(err.contains("tickets") && err.contains("tenant"), "{err}");
}

#[test]
fn cross_space_embedding_comparison_will_not_compile() {
    let src = "\
oracle a = summon \"model-a\"
oracle b = summon \"model-b\"
let ea = a.embed(\"x\")
let eb = b.embed(\"y\")
print similarity(ea, eb)
";
    let err = check_err(src);
    assert!(err.contains("model-a") && err.contains("model-b"), "{err}");
}

#[test]
fn out_of_permit_familiar_action_will_not_compile() {
    let src = format!(
        "{HEADER}
fn delete() requires delete {{ print \"gone\" }}
familiar support_triage(msg) permits {{ invoke triage }} {{
    delete()
}}
support_triage(\"x\")
"
    );
    let err = check_err(&src);
    assert!(
        err.contains("support_triage") && err.contains("delete"),
        "{err}"
    );
}

// --- Litmus: the type constrains generation rather than validating after ---

#[test]
fn deleting_the_type_changes_generation() {
    // Same divine, same seed: the typed run is confined to Disposition; the
    // weakened run (as if the type were deleted) is not, so output differs.
    let src = format!(
        "{HEADER}
divine d: Disposition from (\"x\") using triage with confidence >= 0.0 fallback \"f\"
print d
"
    );
    let typed = run_cfg(
        &src,
        RunConfig {
            seed: 1,
            ..Default::default()
        },
    );
    let weakened = run_cfg(
        &src,
        RunConfig {
            seed: 1,
            weaken_divine: true,
            ..Default::default()
        },
    );
    assert_ne!(typed, weakened, "typed vs weakened generation must differ");
}

// --- Low-confidence fault injection takes the fallback ---

#[test]
fn low_confidence_takes_the_fallback() {
    let out = run_cfg(
        FLAGSHIP,
        RunConfig {
            force_confidence: Some(0.1),
            ..Default::default()
        },
    );
    assert!(out.contains("escalated (fallback)"), "fallback ran: {out}");
    assert!(
        !out.contains("urgency:"),
        "the low-confidence value must not reach enact: {out}"
    );
}
