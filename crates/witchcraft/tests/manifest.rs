//! Manifest BINDING + load-time resolution (change `add-inference-runtime`).
//! Engine selection happens at load; a need that cannot be satisfied under the
//! policy makes the program refuse to start (never a silent policy violation).

use witchcraft::engine::Policy;
use witchcraft::manifest::{Manifest, ResolveError};
use witchcraft::{run_source, RunConfig};

const MANIFEST: &str = r#"
# models are named ONLY here, never in source
[need.TriageReasoner]
engine = "local-mock"
model = "qwen2.5-3b-instruct"
sha256 = "deadbeef"
locality = "local"

[need.CloudReasoner]
engine = "frontier"
model = "some-frontier-model"
locality = "network"

[engine.local-mock]
kind = "mock"

[engine.frontier]
kind = "anthropic"
"#;

fn manifest() -> Manifest {
    Manifest::parse(MANIFEST).expect("manifest parses")
}

#[test]
fn parses_needs_and_engines() {
    let m = manifest();
    assert!(m.needs.contains_key("TriageReasoner"));
    assert_eq!(m.needs["TriageReasoner"].model, "qwen2.5-3b-instruct");
    assert_eq!(m.engines["local-mock"].kind, "mock");
}

#[test]
fn resolves_a_local_mock_need() {
    let m = manifest();
    let engine = m
        .resolve("TriageReasoner", &Policy::default(), 0)
        .expect("local mock resolves under the default on-device policy");
    assert_eq!(engine.describe().backend_id, "mock");
}

#[test]
fn unknown_need_refuses_to_start() {
    let m = manifest();
    match m.resolve("NoSuchNeed", &Policy::default(), 0) {
        Err(e) => assert_eq!(e, ResolveError::UnknownNeed("NoSuchNeed".into())),
        Ok(_) => panic!("an unbound need must refuse to start"),
    }
}

#[test]
fn network_engine_without_permit_refuses_to_start() {
    let m = manifest();
    // Default policy is on-device-only (no permit(network)); a network binding
    // must refuse rather than silently use the network.
    match m.resolve("CloudReasoner", &Policy::default(), 0) {
        Err(e) => assert!(
            matches!(e, ResolveError::NetworkNotPermitted { .. }),
            "expected NetworkNotPermitted, got {e:?}"
        ),
        Ok(_) => panic!("network engine under on-device-only must refuse"),
    }
}

#[test]
fn program_runs_when_manifest_binds_intent_to_mock() {
    let src = "\
type Action = one_of { Draft(reply: glyph), Escalate }
type Disposition = { urgency: spark in 0..10, action: Action }
oracle triage = summon \"TriageReasoner\"
divine d: Disposition from (\"angry customer\") using triage with confidence >= 0.0 fallback \"low confidence\"
print d.urgency
";
    let cfg = RunConfig {
        seed: 3,
        manifest: Some(manifest()),
        ..Default::default()
    };
    let out = run_source(src, cfg).expect("program runs against the bound mock engine");
    assert!(
        out.trim().parse::<f64>().is_ok(),
        "ran and printed an urgency value: {out:?}"
    );
}

#[test]
fn program_refuses_to_start_on_unsatisfiable_policy() {
    // The program names CloudReasoner but does not grant permit(network), so the
    // network binding cannot be satisfied: the program must refuse to start.
    let src = "\
type Action = one_of { Draft(reply: glyph), Escalate }
type Disposition = { urgency: spark in 0..10, action: Action }
oracle cloud = summon \"CloudReasoner\"
divine d: Disposition from (\"angry customer\") using cloud with confidence >= 0.0 fallback \"low confidence\"
print d.urgency
";
    let cfg = RunConfig {
        seed: 3,
        manifest: Some(manifest()),
        ..Default::default()
    };
    let err = run_source(src, cfg).expect_err("must refuse to start");
    let msg = err
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        msg.contains("refuse to start") && msg.contains("permit(network)"),
        "refusal explains the policy violation: {msg}"
    );
}
