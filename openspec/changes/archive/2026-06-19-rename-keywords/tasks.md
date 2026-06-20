## 1. Lexer and parser

- [x] 1.1 Replace token keywords `fn`→`Define`, `print`→`Speak`; add `Listen` keyword/builtin ident resolution
- [x] 1.2 Update parser: function items use `define`; statements use `speak`; reject `fn`/`print`
- [x] 1.3 Update `docs/grammar.ebnf` to match

## 2. Interpreter and type checker

- [x] 2.1 Rename AST/IR nodes and diagnostics (`Fn`→`Define`, `Print`→`Speak`)
- [x] 2.2 Implement `listen(prompt) -> glyph` builtin (blocking stdin, strip newline)
- [x] 2.3 Type-check `listen` as builtin returning `glyph`; `speak` accepts any displayable value

## 3. Codegen and runtime ABI

- [x] 3.1 Rename lowering for `speak`; add `listen` lowering
- [x] 3.2 Add `w_listen` ABI in `witchcraft-runtime`; wire in codegen imports
- [x] 3.3 Test: compiled speak/listen match interpreter (speak via equivalence harness; listen parse + ABI wired; stdin-blocking listen omitted from automated equivalence)

## 4. Mechanical migration

- [x] 4.1 Update ALL `examples/**/*.witch`, integration tests, and `crates/**/tests`
- [x] 4.2 Update README, LANGUAGE.md (naming stopping rule + non-OO note), quick reference
- [x] 4.3 README breaking-change migration table (`fn`→`define`, `print`→`speak`)

## 5. Validation

- [x] 5.1 `openspec validate rename-keywords --strict`
- [x] 5.2 Full `cargo test`; flagship examples `witch check` + `witch run`
