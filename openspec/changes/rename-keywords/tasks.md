## 1. Lexer and parser

- [ ] 1.1 Replace token keywords `fn`→`Define`, `print`→`Speak`; add `Listen` keyword/builtin ident resolution
- [ ] 1.2 Update parser: function items use `define`; statements use `speak`; reject `fn`/`print`
- [ ] 1.3 Update `docs/grammar.ebnf` to match

## 2. Interpreter and type checker

- [ ] 2.1 Rename AST/IR nodes and diagnostics (`Fn`→`Define`, `Print`→`Speak`)
- [ ] 2.2 Implement `listen(prompt) -> glyph` builtin (blocking stdin, strip newline)
- [ ] 2.3 Type-check `listen` as builtin returning `glyph`; `speak` accepts any displayable value

## 3. Codegen and runtime ABI

- [ ] 3.1 Rename lowering for `speak`; add `listen` lowering
- [ ] 3.2 Add `w_listen` ABI in `witchcraft-runtime`; wire in codegen imports
- [ ] 3.3 Test: compiled speak/listen match interpreter

## 4. Mechanical migration

- [ ] 4.1 Update ALL `examples/**/*.witch`, integration tests, and `crates/**/tests`
- [ ] 4.2 Update README, LANGUAGE_GUIDE (naming stopping rule + non-OO note), quick reference
- [ ] 4.3 README breaking-change migration table (`fn`→`define`, `print`→`speak`)

## 5. Validation

- [ ] 5.1 `openspec validate rename-keywords --strict`
- [ ] 5.2 Full `cargo test`; flagship examples `witch check` + `witch run`
