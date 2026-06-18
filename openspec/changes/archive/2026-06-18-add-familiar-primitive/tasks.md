## 1. Familiar as a bounded composite with permits

- [x] 1.1 `familiar <name>(<params>) permits { <permit>, ... } { <body> }` (permit = `<kind>` or `<kind> <param>`, e.g. `invoke triage`, `escalate`, `delete`); `familiar`/`permits` keywords
- [x] 1.2 Documented and named as a composite, not a primitive (§5.5); permits are the elevation-worthy boundary, recorded as part of the checked interface

## 2. Permits are capabilities granted to the body

- [x] 2.1 The body is checked with exactly the permit capabilities active and no others (reuses capability-effects); an action requiring a capability outside `permits` is a permit-violation compile error naming the familiar and the action
- [x] 2.2 Inside a familiar, `divine ... using <oracle>` requires the `invoke <oracle>` permit (the bounded boundary; ambient code outside familiars is unrestricted)
- [x] 2.3 Composition reuses bootstrap: `divine` discharge and `enact` exhaustiveness apply unchanged

## 3. Bounded execution + scope/erasure/tests

- [x] 3.1 Single-pass, deterministic in v0.1 (the §10 firebreak): a familiar body may not contain an unbounded loop — `while` in a familiar body is a missing-bound compile error
- [x] 3.2 Familiars are interpreter-only in v0.x; permits erased at runtime; lowering rejects familiars
- [x] 3.3 Tests: declare; permitted action ok; body cannot exceed permits; out-of-permit (delete) violation names familiar + action; single-pass deterministic; unbounded loop rejected; divine+enact compose; structural-only wording <!-- crates/witchcraft/tests/familiar.rs -->
- [x] 3.4 `openspec validate add-familiar-primitive --strict`; README note
