//! The `grimoire` CLI: the ship path. Compiles a Witchcraft program to a
//! self-contained native executable that runs with no Rust and no `.witch`
//! source.
//!
//!   grimoire build <file.witch> [-o <out>]   typecheck -> lower -> codegen -> link
//!   grimoire check <file.witch>              parse + type-check (no artifact)
//!   grimoire --version
//!
//! `grimoire build` refuses ill-typed programs: no artifact is written and the
//! exit status is non-zero. The produced executable accepts `--seed <n>`, like
//! `witch run`, so compiled and interpreted runs agree for a given seed.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use witchcraft::{check_source, lower_source, Diagnostic};

/// The runtime, linked into every artifact. Embedded at build time (see build.rs)
/// so a shipped `grimoire` is self-contained.
const RUNTIME_LIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libwitchcraft_runtime.a"));

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(msg) => {
            print!("{msg}");
            ExitCode::SUCCESS
        }
        Err(CliError::Usage(msg)) => {
            eprintln!("{msg}\n\n{USAGE}");
            ExitCode::FAILURE
        }
        Err(CliError::Diagnostics(diags)) => {
            for d in &diags {
                eprintln!("{}", d.render());
            }
            ExitCode::FAILURE
        }
        Err(CliError::Build(msg)) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
usage:
  grimoire build  <file.witch> [-o <out>]
  grimoire check  <file.witch>
  grimoire --version";

fn version_string() -> String {
    format!(
        "grimoire {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("GRIMOIRE_TARGET")
    )
}

enum CliError {
    Usage(String),
    Diagnostics(Vec<Diagnostic>),
    Build(String),
}

fn run(args: &[String]) -> Result<String, CliError> {
    let cmd = args
        .first()
        .ok_or_else(|| CliError::Usage("error: missing command".to_string()))?;

    match cmd.as_str() {
        "--version" | "-V" | "version" => Ok(format!("{}\n", version_string())),
        "-h" | "--help" | "help" => Ok(format!("{USAGE}\n")),
        "check" => {
            let file = positional(args)?;
            let src = read(&file)?;
            check_source(&src).map_err(CliError::Diagnostics)?;
            Ok(format!(
                "ok: {file} passed structural checks (this does not assert that inferred values are correct)\n"
            ))
        }
        "build" => build(args),
        other => Err(CliError::Usage(format!("error: unknown command `{other}`"))),
    }
}

fn build(args: &[String]) -> Result<String, CliError> {
    let (file, out) = parse_build_args(args)?;
    let src = read(&file)?;

    // Refuse ill-typed programs before any artifact is produced.
    check_source(&src).map_err(CliError::Diagnostics)?;
    let ir = lower_source(&src).map_err(CliError::Diagnostics)?;
    let object = witchcraft_codegen::compile_object(&ir).map_err(CliError::Build)?;

    let out_path = out.unwrap_or_else(|| default_output(&file));
    link_executable(&object, &out_path).map_err(CliError::Build)?;

    Ok(format!(
        "compiled {} -> {} (run it with no Rust and no .witch source; pass --seed <n> for a fixed seed)\n",
        file,
        out_path.display()
    ))
}

/// Link the compiled object with the embedded runtime into a single native
/// executable, via a C compiler driver (which supplies crt0 + libc).
///
/// The driver and linker are a configurable seam:
///   * `GRIMOIRE_CC` (or `CC`) selects the compiler driver (default `cc`).
///   * `GRIMOIRE_FUSE_LD` (e.g. `lld`) passes `-fuse-ld=<value>`.
///
/// The design's "no toolchain on the build machine" goal ultimately calls for a
/// *bundled* `lld` (so the system `cc` is not required). That binary bundling —
/// and the per-platform SDK handling it implies (e.g. `libSystem` on macOS) — is
/// a packaging concern owned by the distribution change; this seam is where it
/// plugs in. The produced binary is already self-contained (no Rust, no source).
fn link_executable(object: &[u8], out_path: &Path) -> Result<(), String> {
    let work = TempDir::new()?;
    let obj_path = work.path.join("program.o");
    let lib_path = work.path.join("libwitchcraft_runtime.a");
    std::fs::write(&obj_path, object).map_err(|e| format!("writing object: {e}"))?;
    std::fs::write(&lib_path, RUNTIME_LIB).map_err(|e| format!("writing runtime: {e}"))?;

    let driver = std::env::var("GRIMOIRE_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_string());
    let mut cmd = Command::new(&driver);
    if let Ok(fuse_ld) = std::env::var("GRIMOIRE_FUSE_LD") {
        cmd.arg(format!("-fuse-ld={fuse_ld}"));
    }
    cmd.arg(&obj_path)
        .arg(&lib_path)
        .arg("-o")
        .arg(out_path)
        .args(platform_link_args());

    let output = cmd
        .output()
        .map_err(|e| format!("could not run linker `{driver}`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "linking failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// System libraries the Rust runtime staticlib needs, by platform.
fn platform_link_args() -> Vec<&'static str> {
    if cfg!(target_os = "macos") {
        vec![
            "-framework",
            "CoreFoundation",
            "-framework",
            "Security",
            "-liconv",
        ]
    } else if cfg!(target_os = "linux") {
        vec!["-lpthread", "-ldl", "-lm"]
    } else {
        vec![]
    }
}

fn default_output(file: &str) -> PathBuf {
    let stem = Path::new(file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("a");
    PathBuf::from(stem)
}

fn parse_build_args(args: &[String]) -> Result<(String, Option<PathBuf>), CliError> {
    let mut file: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                let raw = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::Usage("error: -o requires a path".to_string()))?;
                out = Some(PathBuf::from(raw));
                i += 2;
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(CliError::Usage(format!("error: unknown flag `{other}`")));
            }
            other => {
                if file.is_some() {
                    return Err(CliError::Usage("error: too many arguments".to_string()));
                }
                file = Some(other.to_string());
                i += 1;
            }
        }
    }
    let file = file.ok_or_else(|| CliError::Usage("error: expected a file path".to_string()))?;
    Ok((file, out))
}

fn positional(args: &[String]) -> Result<String, CliError> {
    args.get(1)
        .cloned()
        .ok_or_else(|| CliError::Usage("error: expected a file path".to_string()))
}

fn read(path: &str) -> Result<String, CliError> {
    std::fs::read_to_string(path)
        .map_err(|e| CliError::Diagnostics(vec![Diagnostic::io(format!("{path}: {e}"))]))
}

/// A throwaway working directory, removed on drop.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Result<Self, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let unique = format!(
            "grimoire-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).map_err(|e| format!("creating temp dir: {e}"))?;
        Ok(TempDir { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
