//! The decoder seam. A `Decoder` generates a value constrained by a grammar,
//! together with a confidence scalar. v0.1 ships exactly one implementation:
//! `MockDecoder`, which is deterministic (seeded) yet honours the grammar
//! token-by-token, so illegal outputs are unreachable and the litmus property
//! holds. Real model backends implement the same trait in v0.2 — no caller
//! changes. No network access occurs here.

use crate::grammar::Grammar;
use crate::value::Value;

pub struct DecodeResult {
    pub value: Value,
    pub confidence: f64,
}

pub trait Decoder {
    fn decode(&mut self, grammar: &Grammar) -> DecodeResult;
}

/// SplitMix64 — small, deterministic, dependency-free PRNG.
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

pub struct MockDecoder {
    rng: Rng,
}

impl MockDecoder {
    pub fn new(seed: u64) -> Self {
        MockDecoder {
            rng: Rng::new(seed),
        }
    }

    fn gen_value(&mut self, grammar: &Grammar) -> Value {
        match grammar {
            Grammar::Number { lo, hi } => {
                let span = (hi - lo).max(0) as usize + 1;
                let n = *lo + self.rng.below(span) as i64;
                Value::Spark(n as f64)
            }
            Grammar::Bool => Value::Bool(self.rng.below(2) == 1),
            Grammar::Text { max_len } => {
                let len = 1 + self.rng.below((*max_len).max(1));
                let alphabet: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
                let mut s = String::new();
                for _ in 0..len {
                    let idx = self.rng.below(alphabet.len());
                    s.push(alphabet[idx] as char);
                }
                Value::Glyph(s)
            }
            Grammar::Record(fields) => {
                let mut out = Vec::new();
                for (n, g) in fields {
                    out.push((n.clone(), self.gen_value(g)));
                }
                Value::Record {
                    fields: out,
                    provenance: None,
                }
            }
            Grammar::OneOf(variants) => {
                // The constraint that makes illegal outputs unreachable: we can
                // only ever pick one of the declared variants.
                let idx = self.rng.below(variants.len().max(1));
                let v = &variants[idx];
                let mut out = Vec::new();
                for (n, g) in &v.fields {
                    out.push((n.clone(), self.gen_value(g)));
                }
                Value::Variant {
                    name: v.name.clone(),
                    fields: out,
                    provenance: None,
                }
            }
        }
    }
}

impl Decoder for MockDecoder {
    fn decode(&mut self, grammar: &Grammar) -> DecodeResult {
        let value = self.gen_value(grammar);
        // A deterministic confidence in [0, 1). Both discharge paths are
        // reachable by choice of seed.
        let confidence = self.rng.next_f64();
        DecodeResult { value, confidence }
    }
}
