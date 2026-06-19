## Why

`dungeon_master.witch` and similar programs need `{ field: expr, ... }` record literals — especially as `divine ... fallback { ... }` values that match the declared output type. Today the parser accepts record types but not record **expressions**; fallbacks are limited to strings, calls, or variants. Record literals are host-side plumbing that makes discharge fallbacks structurally honest (§4: compile-time checkable shape, not runtime parse-and-hope).

## What Changes

- Add record-literal **expressions** `{ name: expr, ... }` in value positions.
- Type-check literals against expected types (required for `divine` fallback against output type `Turn`).
- Interpreter + codegen lowering for `MakeRecord` (or reuse existing record construction path).
- **Non-goals:** record update syntax, spread, optional fields, anonymous records without context.

## Capabilities

### New Capabilities

- `record-literals`: `{ field: expr }` expression form with type inference/checking.

### Modified Capabilities

- `language-grammar`: expression grammar adds record literals (disambiguate from `enact`/`type` blocks via value context).
- `type-system`: literal checking against record types; fallback compatibility with discharge.
- `host-runtime`: construct `Value::Record` from literals.
- `grimoire-codegen`: lower record literals to runtime record builder (mirror existing record field ABI if present).

## Impact

- Depends on `rename-keywords` only for doc/example keyword consistency (can land after or in parallel if examples not touched yet).
- Unblocks typed fallbacks for `enable-dungeon-master-example` before list generation lands (fallback can use record literal with single `exit` field as interim).
