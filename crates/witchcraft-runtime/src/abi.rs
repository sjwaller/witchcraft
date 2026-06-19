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
    intent_ptr: *const u8,
    intent_len: usize,
    input: Value,
    conf_out: *mut f64,
) -> Value {
    let grammar = decode::decode_grammar(std::slice::from_raw_parts(grammar_ptr, grammar_len));
    let intent = str_arg(intent_ptr, intent_len);
    let input_text = value::glyph_to_string(input);

    // Route through the resolved engine (manifest binding) when one exists;
    // otherwise the built-in deterministic Mock decoder serves the need — the
    // offline default, byte-identical to the interpreter's Mock.
    let (value, confidence, prov) = divine_via_engine(intent, &grammar, &input_text)
        .unwrap_or_else(|| {
            let (v, c) = decode::decode(&grammar);
            let prov = Provenance::mock(intent, intent, crate::sink::seed());
            let v = value::set_top_provenance(v, prov.clone());
            (v, c, prov)
        });

    let confidence = crate::sink::force_confidence().unwrap_or(confidence);
    crate::sink::set_last_provenance(prov);
    release(input);
    *conf_out = confidence;
    value
}

#[cfg(feature = "engines")]
fn divine_via_engine(
    intent: &str,
    grammar: &decode::Grammar,
    input: &str,
) -> Option<(Value, f64, crate::value::Provenance)> {
    crate::engines::infer(intent, grammar, input)
}

#[cfg(not(feature = "engines"))]
fn divine_via_engine(
    _intent: &str,
    _grammar: &decode::Grammar,
    _input: &str,
) -> Option<(Value, f64, crate::value::Provenance)> {
    None
}

/// Wrap a value + confidence as an inferred value (undischarged `divine`),
/// taking provenance from the immediately-preceding [`w_divine`] (the engine
/// that produced the value), so it is faithful across engines.
#[no_mangle]
pub extern "C" fn w_make_inferred(inner: Value, confidence: f64) -> Value {
    let prov = crate::sink::last_provenance()
        .unwrap_or_else(|| Provenance::mock("", "", crate::sink::seed()));
    value::inferred(inner, confidence, prov)
}

// ---------- manifest-driven engine resolution (compiled engine-swap) ----------

/// Install a manifest from a TOML subset. Returns 0 on success, 1 on parse error
/// (the message is written to `err_out` as a heap glyph value). Only meaningful
/// when the runtime is built with the `engines` feature.
///
/// # Safety
/// `ptr`/`len` must describe valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_set_manifest(ptr: *const u8, len: usize) -> Value {
    let src = str_arg(ptr, len);
    match set_manifest_impl(src) {
        Ok(()) => value::glyph(""),
        Err(msg) => value::glyph(&msg),
    }
}

#[cfg(feature = "engines")]
fn set_manifest_impl(src: &str) -> Result<(), String> {
    crate::engines::set_manifest(src)
}

#[cfg(not(feature = "engines"))]
fn set_manifest_impl(_src: &str) -> Result<(), String> {
    Err("this build has no engine support (rebuild with the `engines` feature)".to_string())
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

// ---------- lists ----------

/// Finish a list, consuming the builder (field names are discarded; list
/// elements are positional). Each pushed value's reference transfers in.
///
/// # Safety
/// `b` must come from [`w_builder_new`] and not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn w_list_finish(b: *mut Builder) -> Value {
    let b = Box::from_raw(b);
    value::list(b.fields.into_iter().map(|(_, v)| v).collect())
}

// ---------- embeddings ----------

/// Produce a deterministic embedding of `input` (a glyph) in the oracle's space.
/// `space` is the resolved model id; `oracle` is the binding name (provenance).
/// Mirrors the interpreter's `oracle.embed(text)`. Borrows `input`.
///
/// # Safety
/// The name pointers must describe valid UTF-8; `input` must be a live glyph.
#[no_mangle]
pub unsafe extern "C" fn w_embed(
    oracle_ptr: *const u8,
    oracle_len: usize,
    space_ptr: *const u8,
    space_len: usize,
    input: Value,
) -> Value {
    let text = value::glyph_to_string(input);
    let space = str_arg(space_ptr, space_len);
    let oracle = str_arg(oracle_ptr, oracle_len);
    let vector = crate::embed::embed_vector(&text, space);
    let prov = Provenance::mock(oracle, space, crate::sink::seed());
    value::embedding(space, vector, Some(prov))
}

/// Cosine similarity of two embeddings as a spark. Borrows both.
#[no_mangle]
pub extern "C" fn w_similarity(a: Value, b: Value) -> Value {
    let (_, va) = value::embedding_parts(a);
    let (_, vb) = value::embedding_parts(b);
    value::spark(crate::embed::cosine(&va, &vb))
}

/// The `k` nearest candidates to `query` (a list of embeddings), ranked by
/// descending cosine with deterministic tie-breaking. Borrows its arguments;
/// returns a fresh list owning retained references to the chosen candidates.
#[no_mangle]
pub extern "C" fn w_nearest(query: Value, candidates: Value, k: Value) -> Value {
    let (_, qvec) = value::embedding_parts(query);
    let items = value::list_items(candidates);
    let cand_vecs: Vec<Vec<f64>> = items.iter().map(|c| value::embedding_parts(*c).1).collect();
    let k = (value::as_spark(k).max(0.0)) as usize;
    let chosen = crate::embed::rank_top_k(&qvec, &cand_vecs, k);
    let out: Vec<Value> = chosen
        .into_iter()
        .map(|i| {
            retain(items[i]);
            items[i]
        })
        .collect();
    value::list(out)
}

// ---------- governed memory ----------

/// Register a memory store. `has_retention` selects whether `retention` (in
/// logical ticks) applies; `audit` enables the audit log for this store.
///
/// # Safety
/// The name/scope pointers must describe valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_mem_register(
    name_ptr: *const u8,
    name_len: usize,
    scope_ptr: *const u8,
    scope_len: usize,
    has_retention: u8,
    retention: f64,
    audit: u8,
) {
    let retention = if has_retention != 0 {
        Some(retention)
    } else {
        None
    };
    crate::memory::register(
        str_arg(name_ptr, name_len),
        str_arg(scope_ptr, scope_len),
        retention,
        audit != 0,
    );
}

/// Write `value` into a memory store (ownership transfers in).
///
/// # Safety
/// The name pointers must describe valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_mem_write(name_ptr: *const u8, name_len: usize, value: Value) {
    crate::memory::write(str_arg(name_ptr, name_len), value);
}

/// Newest-first retrieval of up to `k` live entries as a list. `method` is the
/// source op name (`recent`/`nearest`) for the audit log. Borrows `k`.
///
/// # Safety
/// The name/method pointers must describe valid UTF-8.
#[no_mangle]
pub unsafe extern "C" fn w_mem_recent(
    name_ptr: *const u8,
    name_len: usize,
    method_ptr: *const u8,
    method_len: usize,
    k: Value,
) -> Value {
    let k = value::as_spark(k).max(0.0) as usize;
    crate::memory::query(
        str_arg(name_ptr, name_len),
        str_arg(method_ptr, method_len),
        k,
    )
}

/// Advance the logical clock by `n` (the `advance` builtin). Borrows `n`.
#[no_mangle]
pub extern "C" fn w_advance(n: Value) {
    let n = value::as_spark(n).max(0.0) as u64;
    crate::memory::advance(n);
}

/// The accumulated audit log as a list of glyphs (the `audit_log` builtin).
#[no_mangle]
pub extern "C" fn w_audit_log() -> Value {
    crate::memory::audit_log()
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
    match c_flag_value(argc, argv, "--seed") {
        Some(s) => s.parse::<u64>().unwrap_or(0),
        None => 0,
    }
}

/// Resolve the program's inference needs against a deployment manifest at process
/// start (the compiled engine-swap), exactly as `witch run --manifest` and the
/// JIT path do. The standalone entry calls this after the seed is set:
///   * with `--manifest <path>` in `argv`, the manifest is installed and every
///     embedded need is resolved under its policy — an unsatisfiable policy
///     (e.g. a network engine without `permit(network)`) makes the process
///     **refuse to start** (message to stderr, exit code 1);
///   * with no `--manifest`, any binding is cleared and the built-in Mock serves
///     each need (the offline default, byte-identical to the interpreter).
///
/// `needs_ptr`/`needs_len` is the codegen-embedded needs blob (see
/// [`crate::encode_needs`]). Models are named only in the manifest.
///
/// # Safety
/// `argv` must be the standard `main` ABI; `needs_ptr`/`needs_len` must describe
/// a blob produced by [`crate::encode_needs`].
#[cfg(feature = "engines")]
#[no_mangle]
pub unsafe extern "C" fn w_setup_manifest(
    argc: i32,
    argv: *const *const std::os::raw::c_char,
    needs_ptr: *const u8,
    needs_len: usize,
) {
    match c_flag_value(argc, argv, "--manifest") {
        Some(path) => {
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!("error: cannot read manifest `{path}`: {e}");
                std::process::exit(1);
            });
            if let Err(e) = crate::engines::set_manifest(&src) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
            let needs = crate::decode_needs(std::slice::from_raw_parts(needs_ptr, needs_len));
            if let Err(e) = crate::engines::resolve_needs(&needs) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        None => crate::engines::clear(),
    }
}

/// Mock-only build: there is no engine bridge, so a `--manifest` request cannot
/// be honoured — refuse loudly rather than silently ignoring the binding.
///
/// # Safety
/// `argv` must be the standard `main` ABI.
#[cfg(not(feature = "engines"))]
#[no_mangle]
pub unsafe extern "C" fn w_setup_manifest(
    argc: i32,
    argv: *const *const std::os::raw::c_char,
    _needs_ptr: *const u8,
    _needs_len: usize,
) {
    if c_flag_value(argc, argv, "--manifest").is_some() {
        eprintln!(
            "error: this binary has no engine support (Mock only); rebuild grimoire with \
             `--features llama` and/or `frontier` to bind a manifest"
        );
        std::process::exit(1);
    }
}

/// Read the value following `flag` in a C `argv`, if present.
///
/// # Safety
/// `argv` must be a valid array of `argc` C strings (the standard `main` ABI).
unsafe fn c_flag_value(
    argc: i32,
    argv: *const *const std::os::raw::c_char,
    flag: &str,
) -> Option<String> {
    if argv.is_null() {
        return None;
    }
    let n = argc.max(0) as isize;
    let mut i = 0isize;
    while i < n {
        let arg = *argv.offset(i);
        if !arg.is_null()
            && std::ffi::CStr::from_ptr(arg).to_bytes() == flag.as_bytes()
            && i + 1 < n
        {
            let val = *argv.offset(i + 1);
            if !val.is_null() {
                if let Ok(s) = std::ffi::CStr::from_ptr(val).to_str() {
                    return Some(s.to_string());
                }
            }
        }
        i += 1;
    }
    None
}
