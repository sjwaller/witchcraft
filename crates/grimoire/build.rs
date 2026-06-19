//! Embed the compiled runtime staticlib into the `grimoire` binary, and capture
//! the build target triple.
//!
//! `grimoire build` links the runtime into every artifact it produces. To keep
//! "install the toolchain, no Rust required" true, `grimoire` carries the runtime
//! with it rather than shelling out to cargo at build time.
//!
//! Cargo only builds a dependency's `rlib` (not its `staticlib`), so we cannot
//! rely on `libwitchcraft_runtime.a` being present or fresh. The runtime is
//! dependency-free (pure `std`), so we compile a fresh `staticlib` directly with
//! `rustc` into `OUT_DIR` — no nested `cargo` (which would deadlock on the
//! package-cache lock), always in sync with the runtime source.
//!
//! With an engine feature (`llama`/`frontier`), the Mock-only staticlib can't
//! carry real engines (they have C/C++ and crate dependencies). So we ALSO build
//! a real-engine runtime staticlib via a nested `cargo` into a SEPARATE target
//! dir (a distinct build lock, so no deadlock) and record its path + native link
//! args for `grimoire build` to link a standalone, manifest-driven executable.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(grimoire_engines)");
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=GRIMOIRE_TARGET={target}");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());

    let runtime_src = manifest
        .join("..")
        .join("witchcraft-runtime")
        .join("src")
        .join("lib.rs");
    let runtime_dir = runtime_src.parent().expect("runtime src dir");
    rerun_if_changed(runtime_dir);

    let lib = out_dir.join("libwitchcraft_runtime.a");
    let status = Command::new(&rustc)
        .args(["--edition", "2021"])
        .args(["--crate-name", "witchcraft_runtime"])
        .args(["--crate-type", "staticlib"])
        .arg("-O")
        .arg(&runtime_src)
        .arg("-o")
        .arg(&lib)
        .status()
        .unwrap_or_else(|e| panic!("failed to run `{rustc}` to build the runtime staticlib: {e}"));
    assert!(
        status.success(),
        "rustc failed to build the runtime staticlib (Mock-only ship path)"
    );

    // When an engine feature is enabled, ALSO produce a real-engine runtime
    // staticlib (with the `engines` bridge + the selected engines, including the
    // bundled `libllama`/`libggml` objects) so `grimoire build` can link a
    // standalone executable that runs real, manifest-selected inference. This is
    // a full cargo build (it resolves `witchcraft` + the engine crates), so it
    // runs into a SEPARATE target dir to avoid contending on the outer build's
    // lock. Default builds skip all of this and stay dependency-free.
    if std::env::var_os("CARGO_FEATURE_ENGINES").is_some() {
        build_engine_runtime(&manifest, &out_dir);
    }
}

/// Build the real-engine runtime staticlib via a nested `cargo` and record its
/// path + native link arguments for `grimoire build` to use. Emits:
///   * `cargo:rustc-cfg=grimoire_engines`
///   * `cargo:rustc-env=GRIMOIRE_ENGINE_RUNTIME_LIB=<path to the staticlib>`
///   * `cargo:rustc-env=GRIMOIRE_ENGINE_LINK_ARGS=<space-separated native libs>`
fn build_engine_runtime(manifest: &Path, out_dir: &Path) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let runtime_pkg = manifest.join("..").join("witchcraft-runtime");
    let runtime_manifest = runtime_pkg.join("Cargo.toml");
    let engine_target = out_dir.join("engine-rt");

    // The engine staticlib also pulls in `witchcraft` (and its engine modules),
    // so a change there must rebuild it too.
    rerun_if_changed(&manifest.join("..").join("witchcraft").join("src"));

    // Mirror the engine features grimoire itself was built with.
    let mut features = vec!["engines"];
    if std::env::var_os("CARGO_FEATURE_LLAMA").is_some() {
        features.push("llama");
    }
    if std::env::var_os("CARGO_FEATURE_FRONTIER").is_some() {
        features.push("frontier");
    }
    let features = features.join(",");

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let release = profile == "release";

    let mut build = Command::new(&cargo);
    build
        .arg("build")
        .args(["--manifest-path", &runtime_manifest.to_string_lossy()])
        .args(["--features", &features])
        .args(["--target-dir", &engine_target.to_string_lossy()]);
    if release {
        build.arg("--release");
    }
    let status = build
        .status()
        .unwrap_or_else(|e| panic!("failed to run nested `{cargo}` for the engine runtime: {e}"));
    assert!(
        status.success(),
        "nested cargo failed to build the engine runtime staticlib"
    );

    let lib = engine_target
        .join(if release { "release" } else { "debug" })
        .join("libwitchcraft_runtime.a");
    assert!(
        lib.exists(),
        "engine runtime staticlib not found at {}",
        lib.display()
    );

    // Discover the native libraries the staticlib must be linked against
    // (frameworks, libc++, libllama's transitive system deps). `rustc` reports
    // these for a `staticlib` crate type via `--print native-static-libs`.
    let link_args = native_static_libs(&cargo, &runtime_manifest, &features, &engine_target);

    println!("cargo:rustc-cfg=grimoire_engines");
    println!(
        "cargo:rustc-env=GRIMOIRE_ENGINE_RUNTIME_LIB={}",
        lib.display()
    );
    println!("cargo:rustc-env=GRIMOIRE_ENGINE_LINK_ARGS={link_args}");
}

/// Ask `rustc` for the native libraries needed to link the engine staticlib.
fn native_static_libs(
    cargo: &str,
    runtime_manifest: &Path,
    features: &str,
    engine_target: &Path,
) -> String {
    let output = Command::new(cargo)
        .arg("rustc")
        .args(["--manifest-path", &runtime_manifest.to_string_lossy()])
        .args(["--features", features])
        .args(["--crate-type", "staticlib"])
        .args(["--target-dir", &engine_target.to_string_lossy()])
        .args(["--", "--print", "native-static-libs"])
        .output()
        .unwrap_or_else(|e| panic!("failed to query native-static-libs: {e}"));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for line in text.lines() {
        if let Some(rest) = line.split("native-static-libs:").nth(1) {
            return rest.trim().to_string();
        }
    }
    String::new()
}

fn rerun_if_changed(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rerun_if_changed(&path);
            } else {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
