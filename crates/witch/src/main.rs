//! The `witch` CLI. Two verbs:
//!
//!   witch check <file.witch>            parse + type-check (no execution)
//!   witch run   <file.witch> [--seed N] check, then run with a fixed seed
//!
//! Exit status is 0 on success and 1 on any diagnostic, so the CLI composes in
//! scripts and CI. Inference is deterministic for a given seed.

use std::process::ExitCode;

use witchcraft::{check_source, run_source, Diagnostic, RunConfig};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(output) => {
            print!("{}", output);
            ExitCode::SUCCESS
        }
        Err(CliError::Usage(msg)) => {
            eprintln!("{}", msg);
            eprintln!();
            eprintln!("{}", USAGE);
            ExitCode::FAILURE
        }
        Err(CliError::Diagnostics(diags)) => {
            for d in &diags {
                eprintln!("{}", d.render());
            }
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
usage:
  witch check   <file.witch>
  witch run     <file.witch> [--seed <n>]
  witch --version";

/// The toolchain version and the triple it was built for.
fn version_string() -> String {
    format!(
        "witch {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("WITCH_TARGET")
    )
}

enum CliError {
    Usage(String),
    Diagnostics(Vec<Diagnostic>),
}

fn run(args: &[String]) -> Result<String, CliError> {
    let cmd = args
        .first()
        .ok_or_else(|| CliError::Usage("error: missing command".to_string()))?;

    match cmd.as_str() {
        "--version" | "-V" | "version" => Ok(format!("{}\n", version_string())),
        "check" => {
            let file = positional(args)?;
            let src = read(&file)?;
            check_source(&src).map_err(CliError::Diagnostics)?;
            Ok(format!(
                "ok: {} passed structural checks (this does not assert that inferred values are correct)\n",
                file
            ))
        }
        "run" => {
            let (file, seed) = parse_run_args(args)?;
            let src = read(&file)?;
            let config = RunConfig {
                seed,
                ..RunConfig::default()
            };
            run_source(&src, config).map_err(CliError::Diagnostics)
        }
        "-h" | "--help" | "help" => Ok(format!("{}\n", USAGE)),
        other => Err(CliError::Usage(format!(
            "error: unknown command `{}`",
            other
        ))),
    }
}

fn positional(args: &[String]) -> Result<String, CliError> {
    args.get(1)
        .cloned()
        .ok_or_else(|| CliError::Usage("error: expected a file path".to_string()))
}

fn parse_run_args(args: &[String]) -> Result<(String, u64), CliError> {
    let mut file: Option<String> = None;
    let mut seed: u64 = 0;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                let raw = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::Usage("error: --seed requires a value".to_string()))?;
                seed = raw
                    .parse()
                    .map_err(|_| CliError::Usage(format!("error: invalid seed `{}`", raw)))?;
                i += 2;
            }
            other if other.starts_with("--") => {
                return Err(CliError::Usage(format!("error: unknown flag `{}`", other)));
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
    Ok((file, seed))
}

fn read(path: &str) -> Result<String, CliError> {
    std::fs::read_to_string(path)
        .map_err(|e| CliError::Diagnostics(vec![Diagnostic::io(format!("{}: {}", path, e))]))
}
