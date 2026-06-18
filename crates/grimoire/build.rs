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

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
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
        "rustc failed to build the runtime staticlib"
    );
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
