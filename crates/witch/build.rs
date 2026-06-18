//! Capture the build target triple so `witch --version` can report it.
//! Cargo sets `TARGET` for build scripts; we re-export it as a compile-time env.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=WITCH_TARGET={target}");
}
