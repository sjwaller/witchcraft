//! The C ABI the Cranelift backend (group 3) emits calls into. These are thin
//! wrappers over the safe Rust API so the symbol names and calling convention
//! are pinned now, while richer constructors (record/variant/inferred) are added
//! alongside the codegen that needs their exact shapes.
//!
//! `Value` is `#[repr(C)]` and 16 bytes, passed and returned by value.

use crate::decode;
use crate::heap::{release, retain};
use crate::value::{self, Provenance, Value};

#[no_mangle]
pub extern "C" fn w_retain(v: Value) {
    retain(v);
}

#[no_mangle]
pub extern "C" fn w_release(v: Value) {
    release(v);
}

#[no_mangle]
pub extern "C" fn w_unit() -> Value {
    value::unit()
}

#[no_mangle]
pub extern "C" fn w_bool(b: bool) -> Value {
    value::boolean(b)
}

#[no_mangle]
pub extern "C" fn w_spark(n: f64) -> Value {
    value::spark(n)
}

/// Build a glyph from a UTF-8 byte range.
///
/// # Safety
/// `ptr` must point to `len` valid, initialised UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn w_glyph(ptr: *const u8, len: usize) -> Value {
    let bytes = std::slice::from_raw_parts(ptr, len);
    let text = std::str::from_utf8(bytes).expect("w_glyph: invalid UTF-8");
    value::glyph(text)
}

/// Render any value to its glyph form (for interpolation / print).
#[no_mangle]
pub extern "C" fn w_render(v: Value) -> Value {
    value::render(v)
}

/// Concatenate two glyph values into a new glyph.
#[no_mangle]
pub extern "C" fn w_concat2(a: Value, b: Value) -> Value {
    value::concat(&[a, b])
}

/// Print a value followed by a newline (the compiled `print`), through the
/// runtime sink (stdout, or the capture buffer when active).
#[no_mangle]
pub extern "C" fn w_print(v: Value) {
    crate::sink::emit_line(&value::display(v));
}

/// Structural equality of two values (the compiled `==`).
#[no_mangle]
pub extern "C" fn w_equals(a: Value, b: Value) -> bool {
    value::equals(a, b)
}

/// The interned variant tag of a value (for `enact` dispatch).
#[no_mangle]
pub extern "C" fn w_variant_tag(v: Value) -> u32 {
    value::variant_tag(v)
}

/// Read a record/variant field, propagating the parent's provenance into the
/// child as the interpreter does. Returns an owned reference.
///
/// # Safety
/// `name_ptr`/`name_len` must describe valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_field(recv: Value, name_ptr: *const u8, name_len: usize) -> Value {
    let name = str_arg(name_ptr, name_len);
    value::field_propagating(recv, name)
}

/// Read a variant payload by positional index (for `enact` bindings). Returns an
/// owned reference.
#[no_mangle]
pub extern "C" fn w_variant_field(recv: Value, index: usize) -> Value {
    let v = value::variant_field(recv, index);
    retain(v);
    v
}

/// A glyph rendering of a value's provenance (empty if none) — what `enact` binds
/// to `provenance`.
#[no_mangle]
pub extern "C" fn w_provenance_glyph(v: Value) -> Value {
    value::provenance_glyph(v)
}

/// The inference primitive. Reconstructs the grammar embedded at this `divine`
/// site (serialised into the artifact), decodes a value the grammar admits,
/// applies fault-injection (forced confidence) if active, attaches provenance
/// (oracle, model, seed), writes the confidence to `conf_out`, and returns the
/// value. Inference happens here, at run time — never at build time.
///
/// # Safety
/// `grammar_ptr`/`grammar_len` must describe valid grammar bytes produced by
/// [`decode::encode`]; the name pointers must be valid UTF-8; `conf_out` must be
/// a valid, writable `f64`.
#[no_mangle]
pub unsafe extern "C" fn w_divine(
    grammar_ptr: *const u8,
    grammar_len: usize,
    oracle_ptr: *const u8,
    oracle_len: usize,
    model_ptr: *const u8,
    model_len: usize,
    conf_out: *mut f64,
) -> Value {
    let grammar = decode::decode_grammar(std::slice::from_raw_parts(grammar_ptr, grammar_len));
    let (value, decoded_conf) = decode::decode(&grammar);
    let confidence = crate::sink::force_confidence().unwrap_or(decoded_conf);
    let prov = Provenance {
        oracle: str_arg(oracle_ptr, oracle_len).to_string(),
        model: str_arg(model_ptr, model_len).to_string(),
        seed: crate::sink::seed(),
    };
    let value = value::set_top_provenance(value, prov);
    *conf_out = confidence;
    value
}

/// Wrap a value + confidence as an inferred value (undischarged `divine`).
///
/// # Safety
/// The name pointers must describe valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_make_inferred(
    inner: Value,
    confidence: f64,
    oracle_ptr: *const u8,
    oracle_len: usize,
    model_ptr: *const u8,
    model_len: usize,
) -> Value {
    let prov = Provenance {
        oracle: str_arg(oracle_ptr, oracle_len).to_string(),
        model: str_arg(model_ptr, model_len).to_string(),
        seed: crate::sink::seed(),
    };
    value::inferred(inner, confidence, prov)
}

// ---------- aggregate construction (records / variants) ----------

/// An incremental record/variant builder. Codegen pushes fields, then finishes.
pub struct Builder {
    fields: Vec<(String, Value)>,
}

#[no_mangle]
pub extern "C" fn w_builder_new() -> *mut Builder {
    Box::into_raw(Box::new(Builder { fields: Vec::new() }))
}

/// Push a named field (taking ownership of `value`'s reference).
///
/// # Safety
/// `b` must come from [`w_builder_new`]; the name pointers must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_builder_push(
    b: *mut Builder,
    name_ptr: *const u8,
    name_len: usize,
    value: Value,
) {
    let b = &mut *b;
    b.fields
        .push((str_arg(name_ptr, name_len).to_string(), value));
}

/// Finish a record, consuming the builder.
///
/// # Safety
/// `b` must come from [`w_builder_new`] and not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn w_record_finish(b: *mut Builder) -> Value {
    let b = Box::from_raw(b);
    value::record(b.fields, None)
}

/// Finish a variant with the given name + interned tag, consuming the builder.
///
/// # Safety
/// `b` must come from [`w_builder_new`]; the name pointers must be valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_variant_finish(
    b: *mut Builder,
    name_ptr: *const u8,
    name_len: usize,
    tag: u32,
) -> Value {
    let b = Box::from_raw(b);
    value::variant(str_arg(name_ptr, name_len), tag, b.fields, None)
}

/// # Safety
/// `ptr`/`len` must describe valid, initialised UTF-8 bytes.
unsafe fn str_arg<'a>(ptr: *const u8, len: usize) -> &'a str {
    std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).expect("invalid UTF-8 argument")
}

// ---------- program entry (compiled executables) ----------

/// Set the run seed (records it for provenance and reseeds the decoder). Called
/// by a compiled executable's entry point before `witch_main`.
#[no_mangle]
pub extern "C" fn w_set_seed(seed: u64) {
    crate::sink::set_seed(seed);
}

/// Parse `--seed <n>` out of a C `argv`, returning the seed (0 if absent or
/// malformed). The compiled executable's entry point calls this so a built
/// artifact accepts the same seed flag as `witch run`.
///
/// # Safety
/// `argv` must be a valid array of `argc` C strings (the standard `main` ABI).
#[no_mangle]
pub unsafe extern "C" fn w_parse_seed(argc: i32, argv: *const *const std::os::raw::c_char) -> u64 {
    if argv.is_null() {
        return 0;
    }
    let n = argc.max(0) as isize;
    let mut i = 0isize;
    while i < n {
        let arg = *argv.offset(i);
        if !arg.is_null() && std::ffi::CStr::from_ptr(arg).to_bytes() == b"--seed" && i + 1 < n {
            let val = *argv.offset(i + 1);
            if !val.is_null() {
                if let Ok(s) = std::ffi::CStr::from_ptr(val).to_str() {
                    if let Ok(parsed) = s.parse::<u64>() {
                        return parsed;
                    }
                }
            }
        }
        i += 1;
    }
    0
}
