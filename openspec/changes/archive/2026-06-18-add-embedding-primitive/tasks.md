## 1. Embedding type + value (space-tagged)

- [x] 1.1 Add `Type::Embedding(space)` (display `embedding@<space>`; assignable only within the same space) and a minimal `Type::List(T)` / list literal `[...]` (needed by `nearest`)
- [x] 1.2 Add `Value::Embedding { space, vector, provenance }` and `Value::List`; structural equality and display
- [x] 1.3 Lexer/parser: list literal `[a, b, c]`

## 2. oracle.embed + same-space operations

- [x] 2.1 `oracle.embed(<glyph>)` produces `embedding@S` where S = the oracle's model id, with provenance (oracle, model, seed); deterministic offline vector
- [x] 2.2 `similarity(a, b)` (same space → spark) and `nearest(query, candidates, k)` (same space → `[embedding@S]`, deterministic tie-break)
- [x] 2.3 Type checker: resolve embed's space from the oracle; cross-space `similarity`/`nearest` is a compile-time error naming both spaces; no implicit bridge

## 3. Scope, erasure, tests

- [x] 3.1 Embeddings/lists are interpreter-only in v0.x; lowering rejects them with a clear diagnostic (Cranelift ship path covers host + divine/enact)
- [x] 3.2 Tests: distinct-space types incompatible; embed yields oracle space + provenance; same-space similarity/nearest deterministic; cross-space comparison fails to compile naming both spaces; structural-only wording <!-- crates/witchcraft/tests/embedding.rs -->
- [x] 3.3 `openspec validate add-embedding-primitive --strict`; README note
