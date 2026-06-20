## Context

`add-embedding-primitive` introduced `Type::List(Box<Type>)` and list literals for `nearest`. The dungeon master example uses English-order `list of T`, not `[T]` — consistent with refinements (`spark in 0..10`) and readable in record fields.

## Goals / Non-Goals

**Goals:**

- Parse `list of T` (unbounded on host side only).
- Parse `list of lo..hi of T` (bounded — required for divine outputs in #4; parsed here, enforced in #4).
- Type-check list literals against `list of T` / bounded list types.
- Display in diagnostics and `Type::display()`.

**Non-Goals:**

- Allow unbounded `list of T` as a `divine` output field (#4 rejects at compile time until bounded).
- `list of T` in expression positions without `of` keyword for literals (literals stay `[...]`).

## Decisions

### D1: Syntax

```
list_type ::= "list" "of" [ bound ] "of" type_expr
            | "list" "of" type_expr          -- sugar for unbounded host lists
bound     ::= spark ".." spark
```

Examples:

- `list of glyph` — host-side, unbounded element type only
- `list of 0..4 of one_of { North, South, East, West }` — bounded (dungeon exits)

**Lean:** bounded form uses **two** `of` keywords (`list of 0..4 of T`) to mirror `spark in 0..10` readability. Unbounded omits the range: `list of T`.

### D2: Internal representation

Extend `Type::List` to carry optional bounds:

```rust
List {
  elem: Box<Type>,
  len: Option<{ lo: i64, hi: i64 }>,  // None = unbounded (host only)
}
```

Assignability: `[a,b]` against `list of T` checks element type; length check against bounds when present.

### D3: §4 note

This change alone does **not** pass the litmus — it names types host-side. The discriminator payoff arrives in #4 when bounds compile into generation grammar.

## Risks / Trade-offs

- **[Two `of` tokens feel verbose]** → Accept for clarity; document in LANGUAGE.md.
- **[Unbounded host lists]** → Allowed for `nearest`/batch; forbidden at `divine` output sites in #4.

## Open Questions

- Should unbounded `list of T` require explicit annotation on `divine` outputs? **Lean:** yes — type checker rule added in #4, not here.
