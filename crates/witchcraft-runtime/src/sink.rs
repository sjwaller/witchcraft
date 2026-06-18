//! Process-side runtime state that is not part of a value: the `print` sink and
//! the inference seed.
//!
//! `print` normally writes to stdout. Tests (and the equivalence harness) can
//! redirect it into an in-memory buffer so compiled output can be compared to
//! the interpreter's without spawning a process. Both are thread-local because
//! the runtime is single-threaded and test threads must stay isolated.

use std::cell::{Cell, RefCell};

thread_local! {
    static CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };
    static SEED: Cell<u64> = const { Cell::new(0) };
}

/// Start capturing `print` output into a buffer instead of stdout.
pub fn begin_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(String::new()));
}

/// Stop capturing and return everything printed since [`begin_capture`].
pub fn end_capture() -> String {
    CAPTURE.with(|c| c.borrow_mut().take().unwrap_or_default())
}

/// Emit one line of program output (newline appended), to the capture buffer if
/// active, else stdout.
pub fn emit_line(s: &str) {
    CAPTURE.with(|c| {
        let mut b = c.borrow_mut();
        match b.as_mut() {
            Some(buf) => {
                buf.push_str(s);
                buf.push('\n');
            }
            None => println!("{}", s),
        }
    });
}

/// Set the inference seed used by the decoder for this run.
pub fn set_seed(seed: u64) {
    SEED.with(|c| c.set(seed));
}

/// The current inference seed.
pub fn seed() -> u64 {
    SEED.with(|c| c.get())
}
