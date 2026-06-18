//! Compiling an output type into a generation grammar. This is the mechanism
//! that makes the type "part of the computation": the decoder can only emit
//! values the grammar admits, so deleting the type (weakening to free text)
//! genuinely changes what is generated (the litmus property, §6.3).

use crate::error::Diagnostic;
use crate::span::Span;
use crate::types::Type;

/// Default upper bound for an unrefined `spark` output (kept small + deterministic).
const SPARK_DEFAULT_HI: f64 = 100.0;
/// Bounded-text length for `glyph` outputs.
const GLYPH_MAX_LEN: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum Grammar {
    /// Inclusive integer range.
    Number {
        lo: i64,
        hi: i64,
    },
    Bool,
    /// Bounded free text — also used to represent an *absent* type constraint.
    Text {
        max_len: usize,
    },
    Record(Vec<(String, Grammar)>),
    OneOf(Vec<GrammarVariant>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrammarVariant {
    pub name: String,
    pub fields: Vec<(String, Grammar)>,
}

/// Compile a type into its generation grammar. If `weaken` is true, the type is
/// treated as absent and everything compiles to free text — this is how we model
/// "delete the type" for the litmus test.
pub fn compile(ty: &Type, weaken: bool, span: Span) -> Result<Grammar, Diagnostic> {
    if weaken {
        return Ok(Grammar::Text {
            max_len: GLYPH_MAX_LEN,
        });
    }
    match ty {
        Type::Spark { lo, hi } => Ok(Grammar::Number {
            lo: lo.unwrap_or(0.0) as i64,
            hi: hi.unwrap_or(SPARK_DEFAULT_HI) as i64,
        }),
        Type::Bool => Ok(Grammar::Bool),
        Type::Glyph => Ok(Grammar::Text {
            max_len: GLYPH_MAX_LEN,
        }),
        Type::Record(fields) => {
            let mut gfields = Vec::new();
            for (n, t) in fields {
                gfields.push((n.clone(), compile(t, false, span)?));
            }
            Ok(Grammar::Record(gfields))
        }
        Type::Sum(variants) => {
            let mut gvars = Vec::new();
            for v in variants {
                let mut gfields = Vec::new();
                for (n, t) in &v.fields {
                    gfields.push((n.clone(), compile(t, false, span)?));
                }
                gvars.push(GrammarVariant {
                    name: v.name.clone(),
                    fields: gfields,
                });
            }
            Ok(Grammar::OneOf(gvars))
        }
        Type::Inferred(_) | Type::Oracle | Type::Unit | Type::Unknown => {
            Err(Diagnostic::type_error(
                format!("type `{}` cannot be a `divine` output type", ty.display()),
                span,
            ))
        }
    }
}
