## 1. Parser and AST

- [x] 1.1 Add `Expr::Record { fields: Vec<(String, Expr)>, span }`
- [x] 1.2 Extend `primary()` to parse `{ ident : expr , ... }` record literals
- [x] 1.3 Tests: parse in `let`, call args, and `fallback` positions; reject ambiguous blocks

## 2. Type checker

- [x] 2.1 Contextual record checking helper (`check_record_literal(expected_type, literal)`)
- [x] 2.2 Wire `divine` fallback checking against `resolve_type(out_ty)`
- [x] 2.3 Diagnostics: missing field, unknown field, wrong type

## 3. Interpreter and codegen

- [x] 3.1 `eval` record literals → `Value::Record`
- [x] 3.2 Lower to IR `MakeRecord` / runtime builder; codegen parity test

## 4. Docs and validation

- [x] 4.1 LANGUAGE_GUIDE: record literals section; note non-OO "plain values"
- [x] 4.2 `openspec validate add-record-literals --strict`; tests in `crates/witchcraft/tests/`
