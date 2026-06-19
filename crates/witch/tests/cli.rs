//! End-to-end CLI tests: invoke the built `witch` binary like a user would.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn witch() -> Command {
    Command::new(env!("CARGO_BIN_EXE_witch"))
}

fn examples_dir() -> PathBuf {
    // crates/witch -> repo root -> examples
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .canonicalize()
        .expect("examples dir")
}

#[test]
fn run_host_example_prints_to_stdout() {
    let out = witch()
        .arg("run")
        .arg(examples_dir().join("host.witch"))
        .output()
        .expect("run witch");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Greetings, witch!"));
    assert!(stdout.contains("arithmetic holds"));
}

#[test]
fn check_passes_on_valid_program() {
    let out = witch()
        .arg("check")
        .arg(examples_dir().join("triage.witch"))
        .output()
        .expect("run witch");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("passed structural checks"));
    // The wording must NOT claim correctness (§8).
    assert!(stdout.contains("does not assert"));
}

#[test]
fn check_fails_on_ungranted_capability() {
    // An operation requiring a capability that no enclosing region grants must
    // fail `witch check` (exit non-zero) and name the missing capability.
    let path = std::env::temp_dir().join(format!("witch_cap_{}.witch", std::process::id()));
    std::fs::write(
        &path,
        "define escalate() requires permit(escalate) { speak \"e\" }\nescalate()\n",
    )
    .expect("write temp program");
    let out = witch().arg("check").arg(&path).output().expect("run witch");
    let _ = std::fs::remove_file(&path);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("permit(escalate)"), "stderr: {stderr}");
}

#[test]
fn run_flagship_example_end_to_end() {
    // The composed §6.3 program type-checks and runs to completion, enacting an
    // action that carries provenance.
    let out = witch()
        .arg("run")
        .arg(examples_dir().join("triage_flagship.witch"))
        .output()
        .expect("run witch");
    assert!(out.status.success(), "flagship should run cleanly");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("recalled scoped history"));
    assert!(stdout.contains("provenance: intent=mock-triage-v1"));
}

#[test]
fn dungeon_master_example_passes_check() {
    let out = witch()
        .arg("check")
        .arg(examples_dir().join("dungeon_master.witch"))
        .output()
        .expect("run witch");
    assert!(
        out.status.success(),
        "dungeon master must type-check: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dungeon_master_runs_with_scripted_stdin() {
    // The interactive listen -> divine -> enact loop, driven by a fixed stdin
    // script under a fixed seed. We assert STRUCTURAL markers (the constrained
    // mechanics), never narrative quality (§8): HP tracking, the bounded `exits`
    // list, and a terminal banner. On EOF `listen` yields empty input, so a
    // short script still drives the loop to a clean finish without blocking.
    let mut child = witch()
        .args(["run"])
        .arg(examples_dir().join("dungeon_master.witch"))
        .args(["--seed", "42"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn witch run");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"look around\ngo north\nsearch\nrest\nwait\n")
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait witch");
    assert!(
        out.status.success(),
        "dungeon master should run to completion: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("=== THE LOCKED TOWER ==="),
        "intro: {stdout}"
    );
    assert!(stdout.contains("HP: 10"), "HP line present: {stdout}");
    assert!(
        stdout.contains("exits:"),
        "constrained exits printed: {stdout}"
    );
    // Some terminal banner is always reached (win, death, or the tower timeout).
    assert!(
        stdout.contains("You win!")
            || stdout.contains("Game over.")
            || stdout.contains("dawn never comes"),
        "a terminal banner is reached: {stdout}"
    );
}

#[test]
fn dungeon_master_same_seed_same_script_is_reproducible() {
    let run = || {
        let mut child = witch()
            .args(["run"])
            .arg(examples_dir().join("dungeon_master.witch"))
            .args(["--seed", "7"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(b"a\nb\nc\n")
            .expect("write");
        let out = child.wait_with_output().expect("wait");
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    assert_eq!(run(), run(), "same seed + same script is deterministic");
}

#[test]
fn same_seed_is_reproducible() {
    let run = |seed: &str| {
        let out = witch()
            .args(["run"])
            .arg(examples_dir().join("triage.witch"))
            .args(["--seed", seed])
            .output()
            .expect("run witch");
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    assert_eq!(run("5"), run("5"));
}

#[test]
fn missing_file_fails_gracefully() {
    let out = witch()
        .arg("run")
        .arg("does-not-exist.witch")
        .output()
        .expect("run witch");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("does-not-exist.witch"));
}

#[test]
fn version_flag_prints_version_and_target() {
    let out = witch().arg("--version").output().expect("run witch");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("witch "));
    // The build target triple is reported in parentheses.
    assert!(stdout.contains('(') && stdout.contains(')'));
    assert!(!stdout.trim().is_empty());
}

#[test]
fn unknown_command_shows_usage() {
    let out = witch().arg("frobnicate").output().expect("run witch");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("usage"));
}
