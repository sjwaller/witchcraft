## Context

Records already exist as types and as inference-generated values. The missing piece is **author-written** record values — the same gap that forces `fallback escalate()` or string fallbacks today. For `divine` discharge, the fallback expression should inhabit the output type structurally; a record literal is the direct host-side spelling.

Witchcraft is **not object-oriented**: a record literal constructs a plain value, not an object with methods. Behaviour stays in `define` functions; no classes.

## Goals / Non-Goals

**Goals:**

- Parse `{ f: e, g: h }` as `Expr::Record`.
- Check fields against a known record type when context supplies one (`divine` fallback vs output type).
- Support nested variants in fields (`outcome: Nothing`, `exits: [North]` once list literals exist).
- Interpreter + compiled parity.

**Non-Goals:**

- Record literals in type positions (types already use `{ ... }` in `type` declarations — parser disambiguation by context).
- Shorthand `{ x }` meaning `{ x: x }` (defer).
- Semantic validation of field *content* (§8).

## Decisions

### D1: Disambiguation — record literal only in expression context

After `:` in a record literal field, parse `expr`. Leading `{` in statement position remains `enact`/`memory`/`within` blocks. In expression position (fallback, `let`, call args), `{` starts a record literal if followed by `ident :`.

*Alternative:* require `record { ... }` keyword — rejected; extra noise.

### D2: Fallback typing is strict

When `divine t: Turn ... fallback { ... }`, the fallback literal MUST type-check as `Turn` (fields present, types compatible). Discharge failure path returns a value of the output shape — not `Unit` unless output is unit.

### D3: §4 discriminator

Record literals are **host-side** sugar for constructing values the type system already understands. The native win is **statically rejecting** a fallback that omits `outcome` or uses wrong field types — a library would fail at runtime when enacting. Not a new primitive; passes §4 as "more statically checkable."

## Risks / Trade-offs

- **[Parser ambiguity with blocks]** → Require `ident :` after `{`; tests for `{` after `fallback` vs block starters.
- **[Codegen record builder]** → Reuse runtime record construction used by decoder output paths.

## Open Questions

- Allow trailing comma in record literals? **Lean:** yes, match type declarations.
