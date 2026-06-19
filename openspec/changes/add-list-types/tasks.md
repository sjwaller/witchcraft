## 1. AST and parser

- [ ] 1.1 Add `TypeExpr::List { elem, lo, hi, span }` to AST
- [ ] 1.2 Extend `type_expr()` for `list of` / `list of lo..hi of`
- [ ] 1.3 Update `docs/grammar.ebnf`

## 2. Type representation and checking

- [ ] 2.1 Extend `Type::List` with optional length bounds; update `display()` and assignability
- [ ] 2.2 `resolve_type` for `TypeExpr::List`
- [ ] 2.3 Check list literals against bounded/unbounded list types

## 3. Tests and docs

- [ ] 3.1 Tests: parse, resolve, literal length errors, element type errors
- [ ] 3.2 LANGUAGE_GUIDE: list types section (host-side; pointer to #4 for divine)
- [ ] 3.3 `openspec validate add-list-types --strict`
