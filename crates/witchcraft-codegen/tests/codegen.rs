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
    assert_equivalent("define add(a, b) { a + b }\nspeak add(2, 3)");
    assert_equivalent("speak 2 + 3 * 4 - 1");
    assert_equivalent("speak 10 / 4");
}

#[test]
fn familiar_lowers_like_a_function_and_matches_the_interpreter() {
    // A familiar that uses only the host language + divine + enact compiles to a
    // function (permits erased, single-pass body) and matches the interpreter.
    let src = "\
type Action = one_of { Draft(reply: glyph), Escalate }
type Disposition = { urgency: spark in 0..10, action: Action }
oracle triage = summon \"mock-triage-v1\"

familiar handle(ticket) permits { invoke triage } {
    divine decision: Disposition
        from (ticket)
        using triage
        with confidence >= 0.0
        fallback \"low\"
    speak \"urgency: ${decision.urgency}\"
    enact decision.action {
        Draft(reply) => { speak \"drafted: ${reply}\" }
        Escalate => { speak \"escalate\" }
    }
}

handle(\"printer on fire\")
";
    assert_equivalent(src);
}

#[test]
fn comparisons_and_equality_and_booleans() {
    assert_equivalent("speak 1 < 2\nspeak 3 <= 3\nspeak 5 > 9\nspeak 4 >= 4");
    assert_equivalent("speak 2 == 2\nspeak 2 == 3\nspeak 1 != 2");
    assert_equivalent("speak true and false\nspeak true or false\nspeak not true");
}

#[test]
fn glyph_interpolation() {
    assert_equivalent("let who = \"witch\"\nspeak \"hi ${who}, ${1 + 1} times\"");
}

#[test]
fn control_flow() {
    assert_equivalent("var n = 0\nwhile n < 3 { speak n n = n + 1 }");
    assert_equivalent("if 2 + 2 == 4 { speak \"ok\" } else { speak \"no\" }");
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
provenance: intent=mock-triage-v1 model=mock-triage-v1 version=mock backend=mock seed=1 sampling=deterministic
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
speak d.urgency
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
speak r
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
speak \"start\"
divine d: Disposition from (\"t\") using o with confidence >= 0.8 fallback \"fb\"
enact d.action {{
    Draft(reply) => {{ speak \"drafted\" }}
    Escalate => {{ speak \"escalated\" }}
    AskClarify(question) => {{ speak \"asked\" }}
}}
speak \"end\"
"
    );
    let ir = lower(&src);

    let injected = run_capture_with(
        &ir,
        RunOptions {
            seed: 1,
            force_confidence: Some(0.0),
            ..Default::default()
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
            ..Default::default()
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

// ---------- capabilities are erased at run time ----------

#[test]
fn capabilities_are_erased_and_compiled_matches_interpreter() {
    // `requires` / `with grant` carry no runtime behaviour: a granted program
    // compiles, runs, and matches the interpreter.
    let src = "\
define escalate(): glyph requires permit(escalate) { \"escalated\" }
with grant permit(escalate) {
    speak escalate()
}
speak \"done\"
";
    assert_equivalent(src);
    assert_eq!(compiled(src, 0), "escalated\ndone\n");
}

#[test]
fn grant_region_compiles_to_its_body() {
    // The same program with the capability scaffolding removed produces identical
    // output — proving the grant region erases to a plain block.
    let with_caps = "\
define act(): glyph requires permit(escalate), scope(tenant) { \"acted\" }
var n = 0
with grant permit(escalate), scope(tenant) {
    while n < 3 { speak \"${n}:${act()}\" n = n + 1 }
}
";
    let stripped = "\
define act(): glyph { \"acted\" }
var n = 0
while n < 3 { speak \"${n}:${act()}\" n = n + 1 }
";
    assert_eq!(compiled(with_caps, 0), compiled(stripped, 0));
    assert_eq!(compiled(with_caps, 0), interpreted(with_caps, 0));
}

// ---------- groups 2-4: lists, embeddings, governed memory, within ----------

#[test]
fn list_literals_match_interpreter() {
    assert_equivalent("let xs = [1, 2, 3]\nspeak xs");
    assert_equivalent("speak [true, false, true]");
    assert_equivalent("let xs = [\"a\", \"b\"]\nspeak xs == [\"a\", \"b\"]");
    assert_equivalent("speak [[1, 2], [3, 4]]");
}

#[test]
fn embedding_similarity_matches_interpreter() {
    // Same text + space ⇒ identical vectors ⇒ cosine 1; different text ⇒ a
    // deterministic similarity that must agree byte-for-byte across paths (this
    // is the guard on the duplicated embed/cosine math).
    assert_equivalent(
        "oracle e = summon \"space-x\"
let a = e.embed(\"hello world\")
let b = e.embed(\"hello world\")
speak similarity(a, b)",
    );
    assert_equivalent(
        "oracle e = summon \"space-x\"
let a = e.embed(\"hello world\")
let b = e.embed(\"goodbye cruel world\")
speak similarity(a, b)",
    );
}

#[test]
fn nearest_ranking_matches_interpreter() {
    // Ordering is not visible through an embedding's display (only its space), so
    // make ranking observable by comparing the result to a recomputed list. Both
    // paths must agree on the boolean regardless of the concrete ranking.
    let src = "\
oracle e = summon \"space-x\"
let q = e.embed(\"urgent payment ticket\")
let c0 = e.embed(\"payment is urgent\")
let c1 = e.embed(\"the weather is nice today\")
let c2 = e.embed(\"urgent ticket\")
let cands = [c0, c1, c2]
speak nearest(q, cands, 2)
speak nearest(q, cands, 1) == [e.embed(\"payment is urgent\")]
";
    assert_equivalent(src);
}

#[test]
fn governed_memory_recency_and_audit_match_interpreter() {
    let src = "\
memory log { scope tenant, retention 24 months, audit required }
within tenant {
    log.write(\"first\")
    log.write(\"second\")
    log.write(\"third\")
    speak log.recent(2)
    speak audit_log()
}
";
    assert_equivalent(src);
}

#[test]
fn memory_retention_expiry_matches_interpreter() {
    // Retention is enforced in logical ticks; `advance` ages the old entry past
    // the window so only the newer one is retrieved — identically on both paths.
    let src = "\
memory log { scope tenant, retention 2 months }
within tenant {
    log.write(\"old\")
    advance(5)
    log.write(\"new\")
    speak log.recent(5)
}
";
    assert_equivalent(src);
}

#[test]
fn within_with_no_memory_erases_to_its_body() {
    let src = "\
var n = 0
within tenant {
    while n < 3 { speak n n = n + 1 }
}
speak \"done\"
";
    assert_equivalent(src);
    assert_eq!(compiled(src, 0), "0\n1\n2\ndone\n");
}

#[test]
fn flagship_compiles_and_matches_interpreter() {
    // The §6.2 acceptance: the four primitives composed in one program now build
    // natively and reproduce `witch run` byte-for-byte on the Mock engine.
    let src = include_str!("../../../examples/triage_flagship.witch");
    for seed in [0u64, 1, 7, 42] {
        assert_eq!(compiled(src, seed), interpreted(src, seed), "seed {seed}");
    }
    assert!(compiled(src, 0).contains("recalled scoped history"));
}

#[test]
fn out_of_scope_memory_access_is_a_compile_error_on_the_native_path() {
    // Lowering runs the type checker first, so an unscoped governed read never
    // reaches codegen — the same compile error the interpreter raises.
    let src = "\
memory tickets { scope tenant }
speak tickets.recent(5)
";
    let err = lower_source(src).expect_err("must not lower");
    let msg = err.iter().map(|d| d.message.clone()).collect::<String>();
    assert!(msg.contains("tickets") && msg.contains("tenant"), "{msg}");
}

#[test]
fn cross_space_embedding_comparison_is_a_compile_error_on_the_native_path() {
    let src = "\
oracle a = summon \"model-a\"
oracle b = summon \"model-b\"
let ea = a.embed(\"x\")
let eb = b.embed(\"y\")
speak similarity(ea, eb)
";
    let err = lower_source(src).expect_err("must not lower");
    let msg = err.iter().map(|d| d.message.clone()).collect::<String>();
    assert!(msg.contains("model-a") && msg.contains("model-b"), "{msg}");
}

#[test]
fn loop_local_list_and_embedding_values_are_reclaimed() {
    // Each iteration allocates a list + embeddings; the emitted refcounting must
    // reclaim them so the live heap returns to baseline after the loop.
    let src = "\
oracle e = summon \"s\"
var n = 0
while n < 2000 {
    let xs = [e.embed(\"${n}\"), e.embed(\"x\")]
    n = n + 1
}
speak \"done\"
";
    let ir = lower_source(src).expect("lower");
    let before = witchcraft_runtime::live_objects();
    let out = run_capture(&ir, 0).expect("run");
    let after = witchcraft_runtime::live_objects();
    assert_eq!(out, "done\n");
    assert_eq!(
        after, before,
        "compiled loop leaked (before={before}, after={after})"
    );
}

#[test]
fn loop_local_heap_is_reclaimed_in_compiled_code() {
    // A glyph is allocated each iteration (via interpolation) and printed. The
    // emitted reference-counting must reclaim it each iteration so the live heap
    // count returns to its baseline after the loop.
    let src = "var n = 0\nwhile n < 5000 { speak \"n=${n}\" n = n + 1 }";
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

// ---------- group 5/8: compiled divine through the Engine contract + manifest ----------

const FLAGSHIP: &str = include_str!("../../../examples/triage_flagship.witch");
const LAPTOP_MANIFEST: &str = include_str!("../../../examples/manifests/triage.laptop.toml");
const CLOUD_MANIFEST: &str = include_str!("../../../examples/manifests/triage.cloud.toml");

/// Compile + run under a deployment manifest, capturing stdout.
fn compiled_with_manifest(src: &str, seed: u64, manifest: &str) -> String {
    run_capture_with(&lower(src), RunOptions::seed(seed).with_manifest(manifest))
        .expect("compiled run under manifest")
}

/// Interpret under the same manifest (the equivalence ground truth).
fn interpreted_with_manifest(src: &str, seed: u64, manifest: &str) -> String {
    use witchcraft::manifest::Manifest;
    interpreted_with(
        src,
        RunConfig {
            seed,
            manifest: Some(Manifest::parse(manifest).expect("manifest parses")),
            ..Default::default()
        },
    )
}

#[test]
fn compiled_divine_through_mock_by_manifest_matches_interpreter() {
    // The compiled flagship routes `divine` through the SAME Engine contract +
    // manifest the interpreter uses (models named only in the manifest). On the
    // Mock binding the compiled native binary must reproduce `witch run`
    // byte-for-byte — the regression guard on the engine seam.
    for seed in [0u64, 1, 7, 42] {
        assert_eq!(
            compiled_with_manifest(FLAGSHIP, seed, LAPTOP_MANIFEST),
            interpreted_with_manifest(FLAGSHIP, seed, LAPTOP_MANIFEST),
            "seed {seed}"
        );
    }
}

#[test]
fn the_same_compiled_flagship_swaps_engine_purely_by_manifest() {
    // Acceptance #3 on the COMPILED (native) path: one compiled flagship, two
    // manifests, two bound models — selected purely by manifest with ZERO source
    // change. Offline both bindings resolve to the deterministic Mock with
    // different model ids (real llama/frontier are the feature-gated tests below);
    // provenance reflects the bound model, proving the swap.
    let laptop = compiled_with_manifest(FLAGSHIP, 1, LAPTOP_MANIFEST);
    let cloud = compiled_with_manifest(FLAGSHIP, 1, CLOUD_MANIFEST);

    assert!(
        laptop.contains("model=local-qwen-2.5-3b"),
        "laptop profile binds the local model: {laptop}"
    );
    assert!(
        cloud.contains("model=cloud-frontier-large"),
        "cloud profile binds the frontier model: {cloud}"
    );
    // The intent is identical across both; only the bound model differs.
    assert!(laptop.contains("intent=mock-triage-v1") && cloud.contains("intent=mock-triage-v1"));

    // And each compiled run matches the interpreter under the same manifest.
    assert_eq!(
        laptop,
        interpreted_with_manifest(FLAGSHIP, 1, LAPTOP_MANIFEST)
    );
    assert_eq!(
        cloud,
        interpreted_with_manifest(FLAGSHIP, 1, CLOUD_MANIFEST)
    );
}

#[test]
fn compiled_program_refuses_to_start_on_unsatisfiable_policy() {
    // A network engine bound without `permit(network)` must refuse to start on
    // the compiled path too — the policy boundary is enforced at load (before any
    // generation), exactly as the interpreter does.
    let src = "\
type Action = one_of { Draft(reply: glyph), Escalate }
type Disposition = { urgency: spark in 0..10, action: Action }
oracle cloud = summon \"CloudReasoner\"
divine d: Disposition from (\"angry customer\") using cloud with confidence >= 0.0 fallback \"low confidence\"
speak d.urgency
";
    let manifest = "\
[need.CloudReasoner]
engine = \"frontier\"
model = \"some-frontier-model\"
locality = \"network\"

[engine.frontier]
kind = \"anthropic\"
";
    let err = run_capture_with(&lower(src), RunOptions::seed(3).with_manifest(manifest))
        .expect_err("must refuse to start");
    assert!(
        err.contains("refuse to start") && err.contains("permit(network)"),
        "refusal explains the policy violation: {err}"
    );
}

/// Acceptance #3 with a REAL local model: the SAME compiled flagship, bound to
/// llama.cpp purely by manifest. Runs live only when `WITCHCRAFT_LLAMA_GGUF`
/// points to a GGUF model (skipped otherwise so offline CI stays green). Built
/// only with `--features llama`.
#[cfg(feature = "llama")]
#[test]
fn compiled_flagship_runs_against_real_llama_by_manifest() {
    let gguf = match std::env::var("WITCHCRAFT_LLAMA_GGUF") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!(
                "skipping live llama acceptance: set WITCHCRAFT_LLAMA_GGUF to a local GGUF model"
            );
            return;
        }
    };
    let manifest = format!(
        "[need.mock-triage-v1]\n\
         engine = \"local\"\n\
         model = \"local-llama\"\n\
         locality = \"local\"\n\n\
         [engine.local]\n\
         kind = \"llama-cpp\"\n\
         gguf = \"{gguf}\"\n"
    );
    let out = compiled_with_manifest(FLAGSHIP, 1, &manifest);
    assert!(
        out.contains("backend=llama"),
        "provenance shows the llama backend (engine selected by manifest): {out}"
    );
}

/// Acceptance #3 with a REAL frontier API: a compiled, network-permitting divine
/// bound to a frontier engine purely by manifest. Runs live only when both
/// `WITCHCRAFT_FRONTIER_KEY_ENV` (the name of the credential env var) and that
/// credential are set. Built only with `--features frontier`.
#[cfg(feature = "frontier")]
#[test]
fn compiled_divine_runs_against_real_frontier_by_manifest() {
    let key_env = match std::env::var("WITCHCRAFT_FRONTIER_KEY_ENV") {
        Ok(name) if !name.is_empty() && std::env::var(&name).is_ok() => name,
        _ => {
            eprintln!(
                "skipping live frontier acceptance: set WITCHCRAFT_FRONTIER_KEY_ENV (and the \
                 credential it names) to run live"
            );
            return;
        }
    };
    let src = "\
type Rating = spark in 0..5
oracle cloud = summon \"CloudReasoner\"
with grant permit(network) {
    divine r: Rating from (\"rate this 0..5\") using cloud with confidence >= 0.0 fallback 0
    speak r
}
";
    let manifest = format!(
        "[need.CloudReasoner]\n\
         engine = \"frontier\"\n\
         model = \"cloud-frontier-large\"\n\
         locality = \"network\"\n\n\
         [engine.frontier]\n\
         kind = \"anthropic\"\n\
         api_key_env = \"{key_env}\"\n"
    );
    let out = compiled_with_manifest(src, 1, &manifest);
    assert!(
        out.trim().parse::<i64>().is_ok(),
        "frontier produced an in-type rating (shape guaranteed): {out:?}"
    );
}
