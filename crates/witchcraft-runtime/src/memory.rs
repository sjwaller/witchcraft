//! The compiled-runtime governed-memory registry (design D2), mirroring the
//! interpreter's in-memory stores, logical clock, retention filter, and audit
//! log so `witch run` and a compiled binary agree on memory behaviour.
//!
//! Scope is enforced *statically* (a `within <scope>` grant; out-of-scope access
//! is a compile error on both paths). What the runtime enforces is retention
//! (expired entries are not returned) and audit (each governed access is logged).
//!
//! State is thread-local and reset at the start of each run (via [`crate::sink::set_seed`]),
//! exactly as the interpreter constructs a fresh store set per run.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::heap::{release, retain};
use crate::value::{self, Value};

struct MemoryStore {
    scope: String,
    retention_ticks: Option<f64>,
    audit_required: bool,
    entries: Vec<(u64, Value)>,
}

thread_local! {
    static MEMORIES: RefCell<HashMap<String, MemoryStore>> = RefCell::new(HashMap::new());
    /// The logical clock shared by memory writes and `advance` (deterministic
    /// retention testing), starting at 0 each run.
    static CLOCK: Cell<u64> = const { Cell::new(0) };
    static AUDIT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Clear the registry, clock, and audit log between runs (the compiled analogue
/// of constructing a fresh interpreter). Releases every stored value.
pub fn reset() {
    MEMORIES.with(|m| {
        for store in m.borrow_mut().drain() {
            for (_, v) in store.1.entries {
                release(v);
            }
        }
    });
    CLOCK.with(|c| c.set(0));
    AUDIT.with(|a| a.borrow_mut().clear());
}

/// Register a memory store. Idempotent re-registration (same name) replaces the
/// store, mirroring re-declaration in a fresh run.
pub fn register(name: &str, scope: &str, retention: Option<f64>, audit_required: bool) {
    MEMORIES.with(|m| {
        m.borrow_mut().insert(
            name.to_string(),
            MemoryStore {
                scope: scope.to_string(),
                retention_ticks: retention,
                audit_required,
                entries: Vec::new(),
            },
        );
    });
}

fn audit(name: &str, method: &str) {
    MEMORIES.with(|m| {
        let mem = m.borrow();
        if let Some(store) = mem.get(name) {
            if store.audit_required {
                let line = format!("memory={} op={} scope={}", name, method, store.scope);
                AUDIT.with(|a| a.borrow_mut().push(line));
            }
        }
    });
}

/// Append `entry` to a store at the current clock tick, then advance the clock by
/// one (matching the interpreter). Takes ownership of `entry`'s reference.
pub fn write(name: &str, entry: Value) {
    audit(name, "write");
    let now = CLOCK.with(|c| c.get());
    MEMORIES.with(|m| {
        if let Some(store) = m.borrow_mut().get_mut(name) {
            store.entries.push((now, entry));
        } else {
            // Unknown memory: nothing to write to; drop the reference.
            release(entry);
        }
    });
    CLOCK.with(|c| c.set(c.get().saturating_add(1)));
}

/// Newest-first retrieval of up to `k` live entries (excluding those older than
/// the declared retention), as a list of retained clones. `method` is the source
/// operation name (`recent`/`nearest`) for the audit log.
pub fn query(name: &str, method: &str, k: usize) -> Value {
    audit(name, method);
    let now = CLOCK.with(|c| c.get());
    let out = MEMORIES.with(|m| {
        let mem = m.borrow();
        let store = match mem.get(name) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let retention = store.retention_ticks;
        let mut live: Vec<(u64, Value)> = store
            .entries
            .iter()
            .filter(|(tick, _)| match retention {
                Some(r) => (now.saturating_sub(*tick)) as f64 <= r,
                None => true,
            })
            .cloned()
            .collect();
        live.sort_by_key(|(tick, _)| std::cmp::Reverse(*tick));
        live.into_iter()
            .take(k)
            .map(|(_, v)| {
                retain(v);
                v
            })
            .collect()
    });
    value::list(out)
}

/// Advance the logical clock by `n` (deterministic retention testing).
pub fn advance(n: u64) {
    CLOCK.with(|c| c.set(c.get().saturating_add(n)));
}

/// The accumulated audit log as a list of glyph values.
pub fn audit_log() -> Value {
    let lines = AUDIT.with(|a| a.borrow().clone());
    value::list(lines.iter().map(|s| value::glyph(s)).collect())
}
