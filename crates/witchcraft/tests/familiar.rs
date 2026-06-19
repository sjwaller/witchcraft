//! Bounded-familiar acceptance tests (§5.4/§5.5): the familiar is a bounded
//! composite whose `permits` set is the checkable boundary. Out-of-permit actions
//! will not compile; the construct is single-pass and deterministic in v0.1.

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

const TYPES: &str = "\
type Action = one_of { Draft(reply: glyph), Escalate }
type Disposition = { urgency: spark in 0..10, action: Action }
oracle triage = summon \"mock-triage-v1\"
";

#[test]
fn declare_and_run_a_bounded_familiar() {
    let src = format!(
        "{TYPES}
familiar support(ticket) permits {{ invoke triage }} {{
    divine decision: Disposition
        from (ticket)
        using triage
        with confidence >= 0.0
        fallback \"low\"
    enact decision.action {{
        Draft(reply) => {{ speak \"drafted: ${{reply}}\" }}
        Escalate => {{ speak \"escalated\" }}
    }}
}}
support(\"printer on fire\")
"
    );
    // Composition of divine + enact under permits runs to completion.
    let out = run(&src);
    assert!(!out.is_empty());
    // Single-pass and deterministic.
    assert_eq!(run(&src), run(&src));
}

#[test]
fn permitted_action_type_checks() {
    let src = format!(
        "{TYPES}
define delete() requires delete {{ speak \"deleted\" }}
familiar danger() permits {{ delete }} {{
    delete()
}}
danger()
"
    );
    assert_eq!(run(&src), "deleted\n");
}

#[test]
fn out_of_permit_action_will_not_compile() {
    // A familiar permitting only `invoke triage` may not perform the `delete`
    // action: a permit violation naming the familiar and the action.
    let src = format!(
        "{TYPES}
define delete() requires delete {{ speak \"gone\" }}
familiar support() permits {{ invoke triage }} {{
    delete()
}}
support()
"
    );
    let err = check_err(&src);
    assert!(err.contains("support"), "names familiar: {err}");
    assert!(err.contains("delete"), "names action: {err}");
}

#[test]
fn divine_without_invoke_permit_is_a_violation() {
    let src = format!(
        "{TYPES}
familiar support(ticket) permits {{ escalate }} {{
    divine decision: Disposition
        from (ticket)
        using triage
        with confidence >= 0.0
        fallback \"low\"
    speak decision.urgency
}}
support(\"x\")
"
    );
    let err = check_err(&src);
    assert!(err.contains("support"), "names familiar: {err}");
    assert!(err.contains("triage"), "names the oracle: {err}");
}

#[test]
fn unbounded_iteration_is_rejected() {
    let src = format!(
        "{TYPES}
familiar loopy() permits {{ }} {{
    while true {{ speak \"x\" }}
}}
"
    );
    let err = check_err(&src);
    assert!(err.contains("loopy"), "names familiar: {err}");
    assert!(err.contains("unbounded loop"), "explains the bound: {err}");
}

#[test]
fn ambient_divine_outside_familiar_needs_no_permit() {
    // Outside a familiar, divine is unrestricted (bootstrap semantics preserved).
    let src = format!(
        "{TYPES}
divine d: Disposition from (\"x\") using triage with confidence >= 0.0 fallback \"low\"
speak d.urgency
"
    );
    assert!(check_source(&src).is_ok());
}
