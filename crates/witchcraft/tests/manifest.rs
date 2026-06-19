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
speak d.urgency
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

const FLAGSHIP: &str = include_str!("../../../examples/triage_flagship.witch");
const LAPTOP_MANIFEST: &str = include_str!("../../../examples/manifests/triage.laptop.toml");
const CLOUD_MANIFEST: &str = include_str!("../../../examples/manifests/triage.cloud.toml");

#[test]
fn flagship_swaps_engine_by_manifest_with_zero_source_change() {
    // The SAME flagship source, run under two manifests, binds the intent to two
    // different models — provenance reflects the bound model, proving the engine
    // is selected purely by manifest (the AI-first + swappable-engine proof, at
    // the interpreter level; real local/network engines are feature-gated).
    let run_with = |manifest_src: &str| -> String {
        let cfg = RunConfig {
            seed: 1,
            manifest: Some(Manifest::parse(manifest_src).expect("manifest parses")),
            ..Default::default()
        };
        run_source(FLAGSHIP, cfg).expect("flagship runs under the manifest")
    };

    let laptop = run_with(LAPTOP_MANIFEST);
    let cloud = run_with(CLOUD_MANIFEST);

    assert!(
        laptop.contains("model=local-qwen-2.5-3b"),
        "laptop profile binds the local model: {laptop}"
    );
    assert!(
        cloud.contains("model=cloud-frontier-large"),
        "cloud profile binds the frontier model: {cloud}"
    );
    // Intent is identical across both; only the bound model differs.
    assert!(laptop.contains("intent=mock-triage-v1") && cloud.contains("intent=mock-triage-v1"));
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
speak d.urgency
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
