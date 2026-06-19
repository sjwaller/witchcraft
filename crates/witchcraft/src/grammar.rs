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
/// Maximum upper bound for a generated list (design D3/risk: the GBNF length
/// disjunction is O(hi), so v0.x caps it; bounded dungeon exits are 0..4).
pub const LIST_MAX_HI: u32 = 16;

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
    /// A bounded homogeneous list: between `lo` and `hi` (inclusive) elements,
    /// each inhabiting `elem`. Generation can only emit a legal cardinality, so
    /// an over-length list is unreachable by construction (the §4 discriminator
    /// for lists). Only the *bounded* form reaches a `divine` output; unbounded
    /// `list of T` is rejected at compile time (see [`compile`]).
    List {
        elem: Box<Grammar>,
        lo: u32,
        hi: u32,
    },
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
        // A list is a legal `divine` output ONLY in its bounded form. The bound
        // compiles into the generation grammar so a too-long list is unreachable
        // (the litmus property for lists). An UNBOUNDED `list of T` has no
        // natural stop during generation and would force validate-after — so it
        // stays a compile error on `divine` outputs (the honest default).
        Type::List {
            elem,
            lo: Some(lo),
            hi: Some(hi),
        } => {
            let lo = *lo as i64;
            let hi = *hi as i64;
            if lo < 0 || hi < 0 || hi < lo {
                return Err(Diagnostic::type_error(
                    format!("list bound {}..{} is not a valid length range", lo, hi),
                    span,
                ));
            }
            if hi as u32 > LIST_MAX_HI {
                return Err(Diagnostic::type_error(
                    format!(
                        "list upper bound {} exceeds the generation cap of {} \
                         (use a smaller bound; large bounds blow up the generation grammar)",
                        hi, LIST_MAX_HI
                    ),
                    span,
                ));
            }
            Ok(Grammar::List {
                elem: Box::new(compile(elem, false, span)?),
                lo: lo as u32,
                hi: hi as u32,
            })
        }
        Type::List { .. } => Err(Diagnostic::type_error(
            format!(
                "unbounded `{}` cannot be a `divine` output type; give an explicit \
                 length bound (e.g. `list of 0..4 of ...`) so generation is bounded",
                ty.display()
            ),
            span,
        )),
        Type::Inferred(_) | Type::Oracle | Type::Embedding(_) | Type::Unit | Type::Unknown => {
            Err(Diagnostic::type_error(
                format!("type `{}` cannot be a `divine` output type", ty.display()),
                span,
            ))
        }
    }
}
