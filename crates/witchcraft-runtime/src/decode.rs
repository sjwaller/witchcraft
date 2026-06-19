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
    /// A bounded homogeneous list — between `lo` and `hi` elements (inclusive),
    /// each inhabiting `elem`. Mirrors the frontend `Grammar::List`; an
    /// over-length list is unreachable during generation.
    List {
        elem: Box<Grammar>,
        lo: u32,
        hi: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrammarVariant {
    pub name: String,
    pub tag: u32,
    pub fields: Vec<(String, Grammar)>,
}

// ---------- serialisation (artifact data section <-> Grammar) ----------
//
// A `divine` site carries its grammar in the compiled artifact as bytes. The
// backend serialises with [`encode`]; the runtime reconstructs with
// [`decode_grammar`] at the call site. The format is a small, self-describing
// little-endian encoding — no external dependency, stable across the two sides.

const TAG_NUMBER: u8 = 0;
const TAG_BOOL: u8 = 1;
const TAG_TEXT: u8 = 2;
const TAG_RECORD: u8 = 3;
const TAG_ONEOF: u8 = 4;
const TAG_LIST: u8 = 5;

/// Serialise a grammar to bytes for embedding in an artifact.
pub fn encode(g: &Grammar) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(g, &mut out);
    out
}

fn encode_into(g: &Grammar, out: &mut Vec<u8>) {
    match g {
        Grammar::Number { lo, hi } => {
            out.push(TAG_NUMBER);
            out.extend_from_slice(&lo.to_le_bytes());
            out.extend_from_slice(&hi.to_le_bytes());
        }
        Grammar::Bool => out.push(TAG_BOOL),
        Grammar::Text { max_len } => {
            out.push(TAG_TEXT);
            out.extend_from_slice(&(*max_len as u64).to_le_bytes());
        }
        Grammar::Record(fields) => {
            out.push(TAG_RECORD);
            encode_fields(fields, out);
        }
        Grammar::OneOf(variants) => {
            out.push(TAG_ONEOF);
            out.extend_from_slice(&(variants.len() as u32).to_le_bytes());
            for v in variants {
                encode_str(&v.name, out);
                out.extend_from_slice(&v.tag.to_le_bytes());
                encode_fields(&v.fields, out);
            }
        }
        Grammar::List { elem, lo, hi } => {
            out.push(TAG_LIST);
            out.extend_from_slice(&lo.to_le_bytes());
            out.extend_from_slice(&hi.to_le_bytes());
            encode_into(elem, out);
        }
    }
}

fn encode_fields(fields: &[(String, Grammar)], out: &mut Vec<u8>) {
    out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    for (name, sub) in fields {
        encode_str(name, out);
        encode_into(sub, out);
    }
}

fn encode_str(s: &str, out: &mut Vec<u8>) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// Reconstruct a grammar from bytes produced by [`encode`].
pub fn decode_grammar(bytes: &[u8]) -> Grammar {
    Reader { bytes, pos: 0 }.grammar()
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.bytes[self.pos];
        self.pos += 1;
        b
    }

    fn u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&self.bytes[self.pos..self.pos + 4]);
        self.pos += 4;
        u32::from_le_bytes(buf)
    }

    fn u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.bytes[self.pos..self.pos + 8]);
        self.pos += 8;
        u64::from_le_bytes(buf)
    }

    fn i64(&mut self) -> i64 {
        self.u64() as i64
    }

    fn str(&mut self) -> String {
        let len = self.u32() as usize;
        let s = String::from_utf8(self.bytes[self.pos..self.pos + len].to_vec())
            .expect("grammar string is valid UTF-8");
        self.pos += len;
        s
    }

    fn fields(&mut self) -> Vec<(String, Grammar)> {
        let n = self.u32() as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let name = self.str();
            out.push((name, self.grammar()));
        }
        out
    }

    fn grammar(&mut self) -> Grammar {
        match self.u8() {
            TAG_NUMBER => Grammar::Number {
                lo: self.i64(),
                hi: self.i64(),
            },
            TAG_BOOL => Grammar::Bool,
            TAG_TEXT => Grammar::Text {
                max_len: self.u64() as usize,
            },
            TAG_RECORD => Grammar::Record(self.fields()),
            TAG_ONEOF => {
                let n = self.u32() as usize;
                let mut variants = Vec::with_capacity(n);
                for _ in 0..n {
                    let name = self.str();
                    let tag = self.u32();
                    let fields = self.fields();
                    variants.push(GrammarVariant { name, tag, fields });
                }
                Grammar::OneOf(variants)
            }
            TAG_LIST => {
                let lo = self.u32();
                let hi = self.u32();
                let elem = Box::new(self.grammar());
                Grammar::List { elem, lo, hi }
            }
            other => panic!("unknown grammar tag {other} in artifact"),
        }
    }
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
        Grammar::List { elem, lo, hi } => {
            // IDENTICAL draw order to the interpreter's `MockDecoder` (length
            // first, then each element) so the compiled and interpreted paths
            // stay byte-equal. A length outside [lo, hi] is unreachable.
            let span = (*hi - *lo) as usize + 1;
            let n = *lo as usize + rng.below(span);
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(gen_value(rng, elem));
            }
            value::list(out)
        }
    }
}
