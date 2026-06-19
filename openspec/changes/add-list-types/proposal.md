## Why

`dungeon_master.witch` declares `exits: list of one_of { North, South, East, West }` — syntax the type parser does not accept today (`Type::List` exists internally for host-side `nearest`, but there is no surface spelling). Host programs need to **name** list types in records and bindings before constrained generation (#4) can use them in `divine` outputs. This change is prerequisite plumbing only — no inference grammar yet.

## What Changes

- Add `list of T` and bounded form `list of lo..hi of T` to type declarations.
- `TypeExpr::List { elem, lo, hi }` → `resolve_type` → `Type::List` (store bounds in `Type::List` metadata or adjacent refinement).
- Display/diagnostics: `list of 0..4 of one_of { ... }`.
- **Non-goals:** list types as `divine` output (change #4); list indexing syntax; list comprehensions.

## Capabilities

### New Capabilities

- `list-types`: surface syntax for homogeneous list types with optional length bounds.

### Modified Capabilities

- `language-grammar`: type declaration grammar for `list of`.
- `type-system`: resolve and display list types; assignability for list literals `[...]` against `list of T`.

## Impact

- Extends `Type::List` (may need `{ elem, min, max }` wrapper — see design).
- Prerequisite for `add-constrained-list-generation`.
- Independent of `rename-keywords` and `add-record-literals` (can parallelize).
