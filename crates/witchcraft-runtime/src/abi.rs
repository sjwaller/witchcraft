//! The C ABI the Cranelift backend (group 3) emits calls into. These are thin
//! wrappers over the safe Rust API so the symbol names and calling convention
//! are pinned now, while richer constructors (record/variant/inferred) are added
//! alongside the codegen that needs their exact shapes.
//!
//! `Value` is `#[repr(C)]` and 16 bytes, passed and returned by value.

use crate::heap::{release, retain};
use crate::value::{self, Value};

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
