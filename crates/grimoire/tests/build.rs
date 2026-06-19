//! End-to-end tests for the ship path (group 5/6): `grimoire build` produces a
//! self-contained native executable whose output matches the interpreter for the
//! same program and seed (the D6 equivalence), and ill-typed programs are refused
//! with no artifact.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use witchcraft::{run_source, RunConfig};

const GRIMOIRE: &str = env!("CARGO_BIN_EXE_grimoire");

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn unique_path(name: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "grimoire-test-{}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed),
        name
    ))
}

/// Build `source_path` to a native executable; panics with the linker/diagnostic
/// output on failure.
fn build(source_path: &Path, out: &Path) {
    let output = Command::new(GRIMOIRE)
        .arg("build")
        .arg(source_path)
        .arg("-o")
        .arg(out)
        .output()
        .expect("run grimoire build");
    assert!(
        output.status.success(),
        "grimoire build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Run a built executable under `seed`, returning its stdout.
fn run_exe(exe: &Path, seed: u64) -> String {
    let output = Command::new(exe)
        .arg("--seed")
        .arg(seed.to_string())
        .output()
        .expect("run compiled executable");
    assert!(
        output.status.success(),
        "compiled executable exited non-zero"
    );
    String::from_utf8(output.stdout).expect("utf-8 stdout")
}

fn interpret(source_path: &Path, seed: u64) -> String {
    let src = std::fs::read_to_string(source_path).expect("read source");
    run_source(
        &src,
        RunConfig {
            seed,
            ..Default::default()
        },
    )
    .expect("interpret")
}

fn assert_compiled_equals_interpreted(example: &str, seeds: &[u64]) {
    let src = examples_dir().join(example);
    let exe = unique_path(&format!("{example}.exe").replace('/', "_"));
    build(&src, &exe);
    for &seed in seeds {
        assert_eq!(
            run_exe(&exe, seed),
            interpret(&src, seed),
            "compiled vs interpreted differ for {example} at seed {seed}"
        );
    }
    let _ = std::fs::remove_file(&exe);
}

#[test]
fn host_example_executable_matches_interpreter() {
    assert_compiled_equals_interpreted("host.witch", &[0, 1, 42]);
}

#[test]
fn triage_example_executable_matches_interpreter() {
    // The §6.3 worked example, AOT-compiled and run as a bare native binary,
    // reproduces the interpreter's inference + provenance for each seed.
    assert_compiled_equals_interpreted("triage.witch", &[1, 7, 42]);
}

#[test]
fn triage_executable_is_self_contained_and_deterministic() {
    let src = examples_dir().join("triage.witch");
    let exe = unique_path("triage-golden.exe");
    build(&src, &exe);
    let expected = "\
urgency: 8
drafted reply: fcrlysheyyil
provenance: intent=mock-triage-v1 model=mock-triage-v1 version=mock backend=mock seed=1 sampling=deterministic
";
    assert_eq!(run_exe(&exe, 1), expected);
    let _ = std::fs::remove_file(&exe);
}

#[test]
fn flagship_executable_matches_interpreter() {
    // §6.2: the flagship — which composes all four primitives (typed embeddings,
    // governed memory under `within`, a bounded familiar, capability discipline)
    // plus divine/enact — now builds with `grimoire build` and reproduces
    // `witch run` byte-for-byte. No separate compilable-only example is needed.
    assert_compiled_equals_interpreted("triage_flagship.witch", &[0, 1, 7, 42]);
}

#[test]
fn ill_typed_program_is_refused_with_no_artifact() {
    let src = unique_path("nonexhaustive.witch");
    std::fs::write(
        &src,
        "\
type Action = one_of { Draft(reply: glyph), Escalate }
type Disposition = { urgency: spark in 0..10, action: Action }
oracle o = summon \"m\"
divine d: Disposition from (\"t\") using o with confidence >= 0.0 fallback \"f\"
enact d.action {
    Draft(reply) => {}
}
",
    )
    .expect("write source");
    let out = unique_path("nonexhaustive.exe");
    let _ = std::fs::remove_file(&out);

    let output = Command::new(GRIMOIRE)
        .arg("build")
        .arg(&src)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("run grimoire build");

    assert!(
        !output.status.success(),
        "ill-typed program must not build successfully"
    );
    assert!(
        !out.exists(),
        "no artifact must be produced for an ill-typed program"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("non-exhaustive"),
        "expected a type diagnostic, got: {stderr}"
    );
    let _ = std::fs::remove_file(&src);
}

/// The headline acceptance for the engine ship path: a `grimoire build --features
/// llama` artifact, run as a BARE standalone process (no JIT, no test harness),
/// performs real grammar-constrained inference against a GGUF model named ONLY in
/// the manifest. Skips unless `WITCHCRAFT_LLAMA_GGUF` points at a local model.
///
///   WITCHCRAFT_LLAMA_GGUF=$PWD/models/<model>.gguf \
///     cargo test -p grimoire --features llama standalone_llama -- --nocapture
#[cfg(feature = "llama")]
#[test]
fn standalone_binary_runs_real_llama_by_manifest() {
    let gguf = match std::env::var("WITCHCRAFT_LLAMA_GGUF") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!(
                "SKIP standalone_binary_runs_real_llama_by_manifest: set WITCHCRAFT_LLAMA_GGUF \
                 to a local GGUF path to run the shipped binary against real weights."
            );
            return;
        }
    };

    // The model is named ONLY in the manifest (the contract); source never moves.
    let manifest = unique_path("triage.llama.toml");
    std::fs::write(
        &manifest,
        format!(
            "[need.mock-triage-v1]\nengine = \"local-qwen\"\nlocality = \"local\"\n\n\
             [engine.local-qwen]\nkind = \"llama\"\ngguf = \"{gguf}\"\n"
        ),
    )
    .expect("write manifest");

    let src = examples_dir().join("triage_flagship.witch");
    let exe = unique_path("flagship-llama.exe");
    build(&src, &exe);

    let output = Command::new(&exe)
        .arg("--manifest")
        .arg(&manifest)
        .args(["--seed", "7"])
        .output()
        .expect("run standalone llama binary");
    assert!(
        output.status.success(),
        "standalone llama binary exited non-zero:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("standalone llama stdout:\n{stdout}");
    // Provenance must show the REAL engine resolved by the manifest, not the Mock.
    assert!(
        stdout.contains("backend=llama"),
        "expected real-llama provenance from the standalone binary, got:\n{stdout}"
    );
    assert!(
        stdout.contains("model=") && stdout.contains(".gguf"),
        "provenance should name the manifest's GGUF model, got:\n{stdout}"
    );

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(&manifest);
}

#[test]
fn version_reports_name_and_target() {
    let output = Command::new(GRIMOIRE)
        .arg("--version")
        .output()
        .expect("run grimoire --version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("grimoire"), "version: {stdout}");
    assert!(
        stdout.contains('('),
        "version should include target: {stdout}"
    );
}
