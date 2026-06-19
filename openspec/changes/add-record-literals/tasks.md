## 1. Parser and AST

- [ ] 1.1 Add `Expr::Record { fields: Vec<(String, Expr)>, span }`
- [ ] 1.2 Extend `primary()` to parse `{ ident : expr , ... }` record literals
- [ ] 1.3 Tests: parse in `let`, call args, and `fallback` positions; reject ambiguous blocks

## 2. Type checker

- [ ] 2.1 Contextual record checking helper (`check_record_literal(expected_type, literal)`)
- [ ] 2.2 Wire `divine` fallback checking against `resolve_type(out_ty)`
- [ ] 2.3 Diagnostics: missing field, unknown field, wrong type

## 3. Interpreter and codegen

- [ ] 3.1 `eval` record literals → `Value::Record`
- [ ] 3.2 Lower to IR `MakeRecord` / runtime builder; codegen parity test

## 4. Docs and validation

- [ ] 4.1 LANGUAGE_GUIDE: record literals section; note non-OO "plain values"
- [ ] 4.2 `openspec validate add-record-literals --strict`; tests in `crates/witchcraft/tests/`
