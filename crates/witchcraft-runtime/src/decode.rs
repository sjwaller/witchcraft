//! The decoder seam, linked into every compiled artifact (design D3/D5). A
//! `divine` site carries its compiled [`Grammar`] in the artifact; at run time
//! the decoder generates a value the grammar admits, plus a confidence scalar.
//!
//! v0.1 ships one implementation: a deterministic, grammar-respecting mock
//! (seeded). Because it honours the grammar token-by-token, illegal outputs are
//! unreachable and the litmus property holds — deleting the type (building with a
//! weakened grammar) genuinely changes generation. Real model backends slot in
//! behind the same `decode` entry in v0.2 with no codegen change.
//!
//! This algorithm is intentionally identical to the interpreter's `MockDecoder`
//! (`witchcraft::decoder`); the compiled/interpreted equivalence tests guard
//! against drift.

use crate::value::{self, Value};

/// A compiled output-type grammar carried in the artifact. Variants carry the
/// interned `tag` assigned by the backend so a decoded variant dispatches
/// correctly through a compiled `enact`.
#[derive(Clone, Debug, PartialEq)]
pub enum Grammar {
    /// Inclusive integer range.
    Number {
        lo: i64,
        hi: i64,
    },
    Bool,
    /// Bounded free text — also represents an *absent* type constraint.
    Text {
        max_len: usize,
    },
    Record(Vec<(String, Grammar)>),
    OneOf(Vec<GrammarVariant>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrammarVariant {
    pub name: String,
    pub tag: u32,
    pub fields: Vec<(String, Grammar)>,
}

/// SplitMix64 — small, deterministic, dependency-free PRNG. Identical to the
/// interpreter's so the same seed yields the same generation.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

thread_local! {
    /// The per-run decoder RNG. Seeded once at the start of a run (mirroring the
    /// interpreter's single `MockDecoder`) so that multiple `divine` sites draw
    /// from one sequence rather than each reseeding.
    static RNG: std::cell::RefCell<Rng> = std::cell::RefCell::new(Rng::new(0));
}

/// (Re)seed the per-run decoder. Called when a run's seed is set.
pub fn reset(seed: u64) {
    RNG.with(|r| *r.borrow_mut() = Rng::new(seed));
}

/// Generate a value constrained by `grammar` from the shared per-run RNG,
/// returning the value (a freshly owned heap value where applicable) and a
/// confidence in `[0, 1)`.
pub fn decode(grammar: &Grammar) -> (Value, f64) {
    RNG.with(|r| {
        let mut rng = r.borrow_mut();
        let value = gen_value(&mut rng, grammar);
        let confidence = rng.next_f64();
        (value, confidence)
    })
}

fn gen_value(rng: &mut Rng, grammar: &Grammar) -> Value {
    match grammar {
        Grammar::Number { lo, hi } => {
            let span = (hi - lo).max(0) as usize + 1;
            let n = *lo + rng.below(span) as i64;
            value::spark(n as f64)
        }
        Grammar::Bool => value::boolean(rng.below(2) == 1),
        Grammar::Text { max_len } => {
            let len = 1 + rng.below((*max_len).max(1));
            let alphabet: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
            let mut s = String::new();
            for _ in 0..len {
                let idx = rng.below(alphabet.len());
                s.push(alphabet[idx] as char);
            }
            value::glyph(&s)
        }
        Grammar::Record(fields) => {
            let mut out = Vec::with_capacity(fields.len());
            for (n, g) in fields {
                out.push((n.clone(), gen_value(rng, g)));
            }
            value::record(out, None)
        }
        Grammar::OneOf(variants) => {
            // The constraint that makes illegal outputs unreachable: only a
            // declared variant can be chosen.
            let idx = rng.below(variants.len().max(1));
            let v = &variants[idx];
            let mut out = Vec::with_capacity(v.fields.len());
            for (n, g) in &v.fields {
                out.push((n.clone(), gen_value(rng, g)));
            }
            value::variant(&v.name, v.tag, out, None)
        }
    }
}
