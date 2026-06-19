## Why

The paper's §7 naming policy ("keep the name, let the plumbing look like plumbing") needs a sharper stopping rule now that Witchcraft grows toward interactive programs. `fn` is a borrowed abbreviation, not a plain word; `print` and `input` sit at the **human boundary** (stdout/stdin) — the seam where the responsibility-holder reads and acts. Renaming them aligns surface syntax with the thesis: evocative vocabulary only where intelligence or the human boundary is involved; plain vocabulary for universal computation. This is a **BREAKING** mechanical rename across the toolchain and all docs/examples — no semantics change.

## What Changes

- **BREAKING:** `fn` → `define` (plain register — full word, not whimsy).
- **BREAKING:** `print` → `speak` (human-facing terminal output on stdout; not generic file/pipe I/O).
- **NEW:** `listen(prompt)` builtin — blocking read of one line from stdin (human-originated input). Evocative register at the human boundary.
- Update lexer, parser, AST, type checker, interpreter, lower/codegen, runtime ABI (`w_speak`, `w_listen`), `docs/grammar.ebnf`, all `.witch` examples, README, and LANGUAGE_GUIDE.
- Document the **naming-philosophy stopping rule** and the **deliberately not object-oriented** data model (plain typed values + functions; capabilities for encapsulation; no classes/inheritance).
- **No other renames.** `let`, `var`, `while`, `if`, `oracle`, `summon`, `divine`, `enact`, etc. stay as-is.

## Capabilities

### New Capabilities

- `human-io-boundary`: `speak` (stdout to the human) and `listen` (stdin from the human); compiled ABI parity for `grimoire build` artifacts.

### Modified Capabilities

- `language-grammar`: host keywords and expression/builtin surface (`define`, `speak`, `listen`; remove `fn`/`print`).
- `host-runtime`: interpreter evaluation of `speak`/`listen`; scoping rules reference `define` not `fn`.
- `grimoire-codegen`: lower `speak`/`listen`; runtime ABI hooks.
- `cli-toolchain`: diagnostics and docs reference new keywords (no user-visible behaviour change beyond syntax).

## Impact

- **Breaking** for every existing `.witch` program and doc snippet using `fn` or `print`.
- Touches `crates/witchcraft` (lexer, parser, token, interp, typeck, lower), `crates/witchcraft-runtime` (sink + new listen ABI), `crates/witchcraft-codegen`, all examples/tests, README, LANGUAGE_GUIDE, `docs/grammar.ebnf`.
- Independent of inference-runtime work; safe to land first. Unblocks `enable-dungeon-master-example` (needs `listen`/`speak`).
