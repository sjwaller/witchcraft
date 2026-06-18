//! The narrow allocation + reference-counting seam (design D8).
//!
//! Every heap value is allocated, retained, and released through this module and
//! nowhere else. That single choke point is what keeps the model reversible: a
//! later region/arena fast-path for hot bounded scopes can replace the body of
//! these functions without touching codegen or the value representation.
//!
//! Witchcraft host values are immutable and acyclic (records/variants are built
//! bottom-up from existing values; nothing can point back), so plain reference
//! counting is complete — no cycle collector is ever required.

use std::cell::Cell;

use crate::value::{Payload, Value};

thread_local! {
    /// Live heap-object count, for leak/reclamation assertions in tests.
    /// Allocation bumps it; reclaiming a payload at refcount zero drops it. It is
    /// a diagnostic, not part of the value semantics. Thread-local because the
    /// runtime is single-threaded and test threads must not see each other's
    /// counts.
    static LIVE: Cell<i64> = const { Cell::new(0) };
}

/// A reference-counted heap object. Single-threaded refcount (`Cell`): compiled
/// Witchcraft programs are single-threaded, and there is no shared mutation.
pub(crate) struct HeapObj {
    pub(crate) rc: Cell<usize>,
    pub(crate) payload: Payload,
}

/// Allocate `payload` on the heap with refcount 1 and return a tagged value
/// pointing at it. The only place heap memory is created.
pub(crate) fn alloc(tag: u64, payload: Payload) -> Value {
    LIVE.with(|c| c.set(c.get() + 1));
    let ptr = Box::into_raw(Box::new(HeapObj {
        rc: Cell::new(1),
        payload,
    }));
    Value {
        tag,
        bits: ptr as u64,
    }
}

#[inline]
pub(crate) fn is_heap(tag: u64) -> bool {
    tag >= crate::value::TAG_GLYPH
}

/// Borrow the heap object behind a heap-tagged value.
///
/// # Safety
/// `v` must be a live heap value produced by [`alloc`] (i.e. retained more times
/// than released). Codegen upholds this; the Rust API upholds it by construction.
pub(crate) unsafe fn obj<'a>(v: Value) -> &'a HeapObj {
    &*(v.bits as *const HeapObj)
}

/// Mutably borrow the heap object behind a heap-tagged value.
///
/// # Safety
/// In addition to [`obj`]'s requirements, the caller must hold the **only**
/// reference (refcount 1) — values are otherwise immutable and shared.
pub(crate) unsafe fn obj_mut<'a>(v: Value) -> &'a mut HeapObj {
    &mut *(v.bits as *mut HeapObj)
}

/// The current refcount of a heap value (1 for a freshly allocated value).
pub(crate) fn refcount(v: Value) -> usize {
    if is_heap(v.tag) {
        unsafe { obj(v) }.rc.get()
    } else {
        0
    }
}

/// Increment the refcount of a heap value; a no-op for unboxed scalars.
pub fn retain(v: Value) {
    if is_heap(v.tag) {
        let o = unsafe { obj(v) };
        o.rc.set(o.rc.get() + 1);
    }
}

/// Decrement the refcount of a heap value; when it reaches zero, release the
/// value's children and free its payload. A no-op for unboxed scalars.
pub fn release(v: Value) {
    if !is_heap(v.tag) {
        return;
    }
    let o = unsafe { obj(v) };
    let n = o.rc.get() - 1;
    if n > 0 {
        o.rc.set(n);
        return;
    }
    // Refcount hit zero: take ownership back and reclaim, decrementing children
    // first so the whole acyclic subgraph is freed exactly once.
    let boxed = unsafe { Box::from_raw(v.bits as *mut HeapObj) };
    match &boxed.payload {
        Payload::Glyph(_) => {}
        Payload::Record { fields, .. } => {
            for (_, child) in fields {
                release(*child);
            }
        }
        Payload::Variant { fields, .. } => {
            for (_, child) in fields {
                release(*child);
            }
        }
        Payload::Inferred { inner, .. } => release(*inner),
        Payload::List(items) => {
            for child in items {
                release(*child);
            }
        }
        Payload::Embedding { .. } => {}
    }
    LIVE.with(|c| c.set(c.get() - 1));
    drop(boxed);
}

/// The number of live heap objects on the current thread. For tests/diagnostics.
pub fn live_objects() -> i64 {
    LIVE.with(|c| c.get())
}
