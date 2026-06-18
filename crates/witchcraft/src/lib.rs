//! Witchcraft v0.1 — an AI-native language whose nativeness lives in the type
//! system. This crate is the whole language: lexer, parser, type checker,
//! tree-walking interpreter, and a deterministic constrained decoder.
//!
//! The headline pipeline is `source -> parse -> check -> run`. Inference flows
//! through the `Decoder` seam (see [`decoder`]); v0.1 ships a deterministic,
//! grammar-respecting mock so the whole language is reproducible and offline.

pub mod ast;
pub mod decoder;
pub mod engine;
pub mod env;
pub mod error;
pub mod grammar;
pub mod interp;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod manifest;
pub mod parser;
pub mod span;
pub mod token;
pub mod typeck;
pub mod types;
pub mod value;

pub use error::{Diagnostic, Stage};
pub use interp::RunConfig;

/// Parse and type-check a program. On success, the program is structurally
/// sound — this is NOT a claim that any inferred value is correct (§8).
pub fn check_source(src: &str) -> Result<(), Vec<Diagnostic>> {
    let program = parser::parse(src).map_err(|d| vec![d])?;
    typeck::check_program(&program)
}

/// Parse, type-check, then run a program, returning captured stdout.
pub fn run_source(src: &str, config: RunConfig) -> Result<String, Vec<Diagnostic>> {
    let program = parser::parse(src).map_err(|d| vec![d])?;
    typeck::check_program(&program)?;
    interp::run(&program, config).map_err(|d| vec![d])
}

/// Parse, type-check, then lower a program to the backend IR. The IR is the
/// target the code generator consumes (see [`ir`] and [`lower`]).
pub fn lower_source(src: &str) -> Result<ir::Program, Vec<Diagnostic>> {
    lower_source_weaken(src, false)
}

/// Like [`lower_source`], but optionally weakens every `divine` output type to
/// free text (the compiled litmus knob — see [`lower::lower_program_weaken`]).
pub fn lower_source_weaken(src: &str, weaken: bool) -> Result<ir::Program, Vec<Diagnostic>> {
    let program = parser::parse(src).map_err(|d| vec![d])?;
    typeck::check_program(&program)?;
    lower::lower_program_weaken(&program, weaken).map_err(|d| vec![d])
}
