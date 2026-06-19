## Context

Bootstrap shipped `fn` and `print` as plain host keywords per an early reading of §7. The paper's own example uses `#` comments and Python-shaped control flow for the mundane — but it also argues that **theming the mundane is a bad uniform** while **evocative names at genuinely new seams** earn their keep. Stdout/stdin are not "plumbing like plumbing"; they are where the human who must answer for the intelligence reads and acts (§9.1). `speak`/`listen` belong at that seam alongside `oracle`/`divine`/`enact`.

`define` replaces `fn` for a different reason: not evocative, but **plain**. `fn` is a borrowed abbreviation; `define` is a full word in the plain register — the same register as `let`, `var`, `while`.

## Goals / Non-Goals

**Goals:**

- Mechanical rename with zero semantic drift for existing constructs.
- Introduce `listen(prompt: glyph) -> glyph` as a blocking stdin read (trailing newline stripped).
- `speak` renders like today's `print` (value display + newline to stdout).
- Compiled artifacts call the same runtime ABI (`w_speak`, `w_listen`).
- Encode the stopping rule and non-OO note in LANGUAGE_GUIDE.

**Non-Goals:**

- Generic file I/O, pipes, or sockets (future plain plumbing, not `speak`/`listen`).
- Capabilities/permits on `listen`/`speak` in v0.x (human-boundary I/O is ambient for CLI programs; revisit if embedding in familiars needs bounds).
- Deprecation aliases for `fn`/`print` (clean break; pre-1.0).

## Decisions

### D1: Two registers, one stopping rule

| Register | Keywords | Rule |
|---|---|---|
| **Plain** | `define`, `let`, `var`, `while`, `if`, `else`, `return`, … | Universal computation; never themed |
| **Evocative** | `oracle`, `summon`, `divine`, `enact`, `memory`, `familiar`, `speak`, `listen`, … | Intelligence seam or human boundary only |

**Stopping rule (docs):** abbreviations become plain words; plain words and pure-computation keywords stay as-is; evocative names are reserved for the intelligence/human-boundary seam.

### D2: `speak` is stdout-only, human-facing

`speak expr` writes to stdout with newline. It is **not** a generic write. Rationale: keeps the evocative name honest — you speak *to the human*, not to a file descriptor.

### D3: `listen` is stdin-only, blocking

`listen(prompt)` prints nothing by itself; the prompt is a **glyph argument** used for display context (caller may `speak prompt` first, or pass `"> "` inline). Signature lean: `listen(prompt: glyph) -> glyph` reads one line from stdin, strips trailing `\n`, returns the glyph. Blocking is acceptable for `witch run` and compiled CLI binaries.

*Alternative:* zero-arg `listen()` — rejected; dungeon_master and REPL patterns want a visible prompt glyph.

### D4: Not object-oriented (design note for docs)

Witchcraft data is plain typed values; behaviour is functions (`define`); encapsulation is capabilities (`scope`, `permits`, `requires`). No classes, no inheritance. Future polymorphism, if any, is traits/abilities — never OO inheritance. Rationale (thesis): grammar-constrain **values**, not behaviour/objects; one function concept keeps `define` unambiguous.

### D5: Single commit-style migration

No dual-keyword period. Update all examples and tests in the same change. `openspec validate --strict` + full test suite gate.

## Risks / Trade-offs

- **[Breaking all existing programs]** → Accept pre-1.0; README migration note with search-replace table.
- **[Compiled/runtime ABI churn]** → Add `w_listen` beside existing print sink; version bump in packaging if needed.
- **[Evocative I/O feels themed]** → Justified by human-boundary argument; `while`/`let` explicitly not renamed.

## Migration Plan

1. Land compiler/runtime/codegen + ABI.
2. Mechanical replace in examples, tests, README, LANGUAGE_GUIDE, grammar.ebnf.
3. Verify `witch check`/`witch run`/`grimoire build` on flagship examples.
4. README breaking-change note: `fn`→`define`, `print`→`speak`; new `listen`.

## Open Questions

- Should `listen` echo the prompt, or only return the line? **Lean:** caller controls output (`speak prompt` then `listen("")` or pass prompt for future structured REPL) — document idiomatic `speak "> "; let line = listen("")` vs single-call with prompt reserved for compiled prompt string metadata only.
- **Resolved lean:** `listen(prompt)` does not implicitly print; examples use `speak "${prompt}${listen("")}"` pattern OR we allow `listen` to optionally write prompt to stdout before read — **lean toward explicit `speak` first** to keep `speak` the only stdout writer.
