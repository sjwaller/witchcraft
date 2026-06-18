//! Diagnostics. Every user-facing error carries a stage, a message, and (where
//! available) a source position. User errors never panic; panics are reserved
//! for internal invariants.

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Lex,
    Parse,
    Type,
    Runtime,
    Io,
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Stage::Lex => "lex",
            Stage::Parse => "syntax",
            Stage::Type => "type",
            Stage::Runtime => "runtime",
            Stage::Io => "io",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub stage: Stage,
    pub message: String,
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn new(stage: Stage, message: impl Into<String>, span: Option<Span>) -> Self {
        Diagnostic {
            stage,
            message: message.into(),
            span,
        }
    }

    pub fn lex(message: impl Into<String>, span: Span) -> Self {
        Diagnostic::new(Stage::Lex, message, Some(span))
    }

    pub fn parse(message: impl Into<String>, span: Span) -> Self {
        Diagnostic::new(Stage::Parse, message, Some(span))
    }

    pub fn type_error(message: impl Into<String>, span: Span) -> Self {
        Diagnostic::new(Stage::Type, message, Some(span))
    }

    pub fn runtime(message: impl Into<String>, span: Span) -> Self {
        Diagnostic::new(Stage::Runtime, message, Some(span))
    }

    pub fn io(message: impl Into<String>) -> Self {
        Diagnostic::new(Stage::Io, message, None)
    }

    /// Human-readable, positioned rendering for the CLI.
    pub fn render(&self) -> String {
        match self.span {
            Some(span) => format!(
                "error[{}] at {}: {}",
                self.stage.label(),
                span,
                self.message
            ),
            None => format!("error[{}]: {}", self.stage.label(), self.message),
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.render())
    }
}
