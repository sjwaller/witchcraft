//! The compiled runtime value representation (design D8).
//!
//! A value is a 16-byte tagged pair `{ tag, bits }`. Scalars are **unboxed**
//! (the payload lives in `bits`); heap kinds (glyph, record, variant, inferred)
//! store a pointer to a reference-counted [`crate::heap::HeapObj`] in `bits`.
//! `Value` is `Copy`: copying does NOT change refcounts — callers (codegen, and
//! the Rust API here) explicitly [`retain`](crate::heap::retain) /
//! [`release`](crate::heap::release). This mirrors what native code emits.
//!
//! Logical and display semantics deliberately match the interpreter's
//! `witchcraft::value::Value` so the compiled and interpreted paths agree (D6).

use crate::heap::{alloc, is_heap, obj, obj_mut, refcount, retain};

pub const TAG_UNIT: u64 = 0;
pub const TAG_BOOL: u64 = 1;
pub const TAG_SPARK: u64 = 2;
pub const TAG_GLYPH: u64 = 3;
pub const TAG_RECORD: u64 = 4;
pub const TAG_VARIANT: u64 = 5;
pub const TAG_INFERRED: u64 = 6;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Value {
    pub tag: u64,
    pub bits: u64,
}

/// Where an inferred value came from. Mirrors the interpreter's `Provenance`.
#[derive(Clone, Debug, PartialEq)]
pub struct Provenance {
    pub oracle: String,
    pub model: String,
    pub seed: u64,
}

impl Provenance {
    pub fn render(&self) -> String {
        // Mirrors the interpreter's `Provenance::render`. The compiled path is
        // Mock-only in v0.2 (real engines attach in `complete-native-compile`),
        // so version/backend/sampling are the Mock constants here.
        format!(
            "intent={} model={} version=mock backend=mock seed={} sampling=deterministic",
            self.oracle, self.model, self.seed
        )
    }
}

/// Heap payloads. Field names ride along so field-by-name access and display
/// match the interpreter.
pub(crate) enum Payload {
    Glyph(String),
    Record {
        fields: Vec<(String, Value)>,
        provenance: Option<Provenance>,
    },
    Variant {
        name: String,
        tag: u32,
        fields: Vec<(String, Value)>,
        provenance: Option<Provenance>,
    },
    Inferred {
        inner: Value,
        confidence: f64,
        provenance: Provenance,
    },
}

// ---------- scalar constructors (unboxed, no allocation) ----------

#[inline]
pub fn unit() -> Value {
    Value {
        tag: TAG_UNIT,
        bits: 0,
    }
}

#[inline]
pub fn boolean(b: bool) -> Value {
    Value {
        tag: TAG_BOOL,
        bits: b as u64,
    }
}

#[inline]
pub fn spark(n: f64) -> Value {
    Value {
        tag: TAG_SPARK,
        bits: n.to_bits(),
    }
}

// ---------- heap constructors (each returns a value with refcount 1) ----------

pub fn glyph(text: &str) -> Value {
    alloc(TAG_GLYPH, Payload::Glyph(text.to_string()))
}

/// Build a record taking ownership of one reference to each field value.
pub fn record(fields: Vec<(String, Value)>, provenance: Option<Provenance>) -> Value {
    alloc(TAG_RECORD, Payload::Record { fields, provenance })
}

/// Build a variant taking ownership of one reference to each payload value.
pub fn variant(
    name: &str,
    tag: u32,
    fields: Vec<(String, Value)>,
    provenance: Option<Provenance>,
) -> Value {
    alloc(
        TAG_VARIANT,
        Payload::Variant {
            name: name.to_string(),
            tag,
            fields,
            provenance,
        },
    )
}

/// Wrap an inner value (whose reference this takes ownership of) as inferred.
pub fn inferred(inner: Value, confidence: f64, provenance: Provenance) -> Value {
    alloc(
        TAG_INFERRED,
        Payload::Inferred {
            inner,
            confidence,
            provenance,
        },
    )
}

// ---------- accessors ----------

#[inline]
pub fn as_bool(v: Value) -> bool {
    debug_assert_eq!(v.tag, TAG_BOOL);
    v.bits != 0
}

#[inline]
pub fn as_spark(v: Value) -> f64 {
    debug_assert_eq!(v.tag, TAG_SPARK);
    f64::from_bits(v.bits)
}

/// The text of a glyph value as an owned string.
pub fn glyph_to_string(v: Value) -> String {
    match &unsafe { obj(v) }.payload {
        Payload::Glyph(s) => s.clone(),
        _ => panic!("glyph_to_string on non-glyph value"),
    }
}

/// Read a record/variant field by name. Returns a borrowed copy of the child
/// value; the caller must [`retain`](crate::heap::retain) it to keep it past the
/// parent's lifetime.
pub fn field(v: Value, name: &str) -> Value {
    let fields = match &unsafe { obj(v) }.payload {
        Payload::Record { fields, .. } | Payload::Variant { fields, .. } => fields,
        _ => panic!("field access on non-aggregate value"),
    };
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, c)| *c)
        .unwrap_or_else(|| panic!("no field `{}`", name))
}

/// The interned tag of a variant value (for `enact` dispatch).
pub fn variant_tag(v: Value) -> u32 {
    match &unsafe { obj(v) }.payload {
        Payload::Variant { tag, .. } => *tag,
        _ => panic!("variant_tag on non-variant value"),
    }
}

/// A variant payload by positional index (for `enact` bindings).
pub fn variant_field(v: Value, index: usize) -> Value {
    match &unsafe { obj(v) }.payload {
        Payload::Variant { fields, .. } => fields[index].1,
        _ => panic!("variant_field on non-variant value"),
    }
}

pub fn inferred_inner(v: Value) -> Value {
    match &unsafe { obj(v) }.payload {
        Payload::Inferred { inner, .. } => *inner,
        _ => panic!("inferred_inner on non-inferred value"),
    }
}

pub fn inferred_confidence(v: Value) -> f64 {
    match &unsafe { obj(v) }.payload {
        Payload::Inferred { confidence, .. } => *confidence,
        _ => panic!("inferred_confidence on non-inferred value"),
    }
}

/// The provenance carried by a record/variant/inferred value, if any.
pub fn provenance(v: Value) -> Option<Provenance> {
    if !is_heap(v.tag) {
        return None;
    }
    match &unsafe { obj(v) }.payload {
        Payload::Record { provenance, .. } | Payload::Variant { provenance, .. } => {
            provenance.clone()
        }
        Payload::Inferred { provenance, .. } => Some(provenance.clone()),
        Payload::Glyph(_) => None,
    }
}

// ---------- provenance threading (must match the interpreter) ----------

/// Attach provenance to a freshly produced record/variant **in place**. Safe
/// only when the caller holds the sole reference (e.g. the value just returned by
/// the decoder). Scalars and glyphs are returned unchanged. Mirrors the
/// interpreter's `attach_provenance` (top level only).
pub fn set_top_provenance(v: Value, prov: Provenance) -> Value {
    if !matches!(v.tag, TAG_RECORD | TAG_VARIANT) {
        return v;
    }
    debug_assert_eq!(refcount(v), 1, "set_top_provenance requires sole ownership");
    let payload = &mut unsafe { obj_mut(v) }.payload;
    match payload {
        Payload::Record { provenance, .. } | Payload::Variant { provenance, .. } => {
            *provenance = Some(prov);
        }
        _ => {}
    }
    v
}

/// Read a field and propagate the parent's provenance into the child if the
/// child is a record/variant that carries none — mirrors the interpreter's
/// `propagate_provenance` on field access. Returns an owned reference.
pub fn field_propagating(recv: Value, name: &str) -> Value {
    let parent_prov = provenance(recv);
    let child = field(recv, name);

    if let Some(p) = parent_prov {
        let needs = match child.tag {
            TAG_RECORD | TAG_VARIANT => provenance(child).is_none(),
            _ => false,
        };
        if needs {
            return clone_with_provenance(child, p);
        }
    }
    retain(child);
    child
}

/// Build a new record/variant with the same (retained) fields plus `prov`.
fn clone_with_provenance(v: Value, prov: Provenance) -> Value {
    match &unsafe { obj(v) }.payload {
        Payload::Record { fields, .. } => {
            let cloned = retain_fields(fields);
            record(cloned, Some(prov))
        }
        Payload::Variant {
            name, tag, fields, ..
        } => {
            let (name, tag) = (name.clone(), *tag);
            let cloned = retain_fields(fields);
            variant(&name, tag, cloned, Some(prov))
        }
        _ => {
            retain(v);
            v
        }
    }
}

fn retain_fields(fields: &[(String, Value)]) -> Vec<(String, Value)> {
    fields
        .iter()
        .map(|(n, c)| {
            retain(*c);
            (n.clone(), *c)
        })
        .collect()
}

/// A glyph rendering of a value's provenance (empty glyph if it has none).
pub fn provenance_glyph(v: Value) -> Value {
    match provenance(v) {
        Some(p) => glyph(&p.render()),
        None => glyph(""),
    }
}

// ---------- equality (must match the interpreter's `Value` PartialEq) ----------

/// Structural equality, mirroring `witchcraft::value::Value`'s derived `PartialEq`
/// so the compiled `==` agrees with the interpreter. Provenance participates in
/// equality, exactly as in the interpreter.
pub fn equals(a: Value, b: Value) -> bool {
    if a.tag != b.tag {
        return false;
    }
    match a.tag {
        TAG_UNIT => true,
        TAG_BOOL => as_bool(a) == as_bool(b),
        TAG_SPARK => as_spark(a) == as_spark(b),
        TAG_GLYPH => glyph_to_string(a) == glyph_to_string(b),
        TAG_RECORD => {
            let (fa, pa) = match &unsafe { obj(a) }.payload {
                Payload::Record { fields, provenance } => (fields, provenance),
                _ => unreachable!(),
            };
            let (fb, pb) = match &unsafe { obj(b) }.payload {
                Payload::Record { fields, provenance } => (fields, provenance),
                _ => unreachable!(),
            };
            pa == pb && fields_equal(fa, fb)
        }
        TAG_VARIANT => {
            let (na, ta, fa, pa) = match &unsafe { obj(a) }.payload {
                Payload::Variant {
                    name,
                    tag,
                    fields,
                    provenance,
                } => (name, tag, fields, provenance),
                _ => unreachable!(),
            };
            let (nb, tb, fb, pb) = match &unsafe { obj(b) }.payload {
                Payload::Variant {
                    name,
                    tag,
                    fields,
                    provenance,
                } => (name, tag, fields, provenance),
                _ => unreachable!(),
            };
            na == nb && ta == tb && pa == pb && fields_equal(fa, fb)
        }
        TAG_INFERRED => {
            let (ia, ca, pa) = match &unsafe { obj(a) }.payload {
                Payload::Inferred {
                    inner,
                    confidence,
                    provenance,
                } => (*inner, *confidence, provenance),
                _ => unreachable!(),
            };
            let (ib, cb, pb) = match &unsafe { obj(b) }.payload {
                Payload::Inferred {
                    inner,
                    confidence,
                    provenance,
                } => (*inner, *confidence, provenance),
                _ => unreachable!(),
            };
            ca == cb && pa == pb && equals(ia, ib)
        }
        _ => false,
    }
}

fn fields_equal(a: &[(String, Value)], b: &[(String, Value)]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|((na, va), (nb, vb))| na == nb && equals(*va, *vb))
}

// ---------- glyph operations ----------

/// Render any value to its glyph text (for interpolation and `print`).
pub fn render(v: Value) -> Value {
    glyph(&display(v))
}

/// Concatenate glyph values left to right into a new glyph.
pub fn concat(parts: &[Value]) -> Value {
    let mut out = String::new();
    for p in parts {
        out.push_str(&glyph_to_string(*p));
    }
    glyph(&out)
}

// ---------- display (must match the interpreter's `Value::display`) ----------

pub fn display(v: Value) -> String {
    match v.tag {
        TAG_UNIT => "()".to_string(),
        TAG_BOOL => as_bool(v).to_string(),
        TAG_SPARK => fmt_num(as_spark(v)),
        TAG_GLYPH => glyph_to_string(v),
        TAG_RECORD => {
            let fields = match &unsafe { obj(v) }.payload {
                Payload::Record { fields, .. } => fields,
                _ => unreachable!(),
            };
            let inner: Vec<String> = fields
                .iter()
                .map(|(n, c)| format!("{}: {}", n, display(*c)))
                .collect();
            format!("{{ {} }}", inner.join(", "))
        }
        TAG_VARIANT => {
            let (name, fields) = match &unsafe { obj(v) }.payload {
                Payload::Variant { name, fields, .. } => (name, fields),
                _ => unreachable!(),
            };
            if fields.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(n, c)| format!("{}: {}", n, display(*c)))
                    .collect();
                format!("{}({})", name, inner.join(", "))
            }
        }
        TAG_INFERRED => {
            let (inner, confidence) = match &unsafe { obj(v) }.payload {
                Payload::Inferred {
                    inner, confidence, ..
                } => (*inner, *confidence),
                _ => unreachable!(),
            };
            format!(
                "Inferred({}, confidence={})",
                display(inner),
                fmt_num(confidence)
            )
        }
        _ => panic!("display on unknown tag {}", v.tag),
    }
}

/// Number formatting identical to the interpreter's `fmt_num`.
pub fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}
