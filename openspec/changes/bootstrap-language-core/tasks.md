## 1. Project scaffolding

- [ ] 1.1 Create a Cargo workspace with a `witchcraft` library crate and a `witch` binary crate
- [ ] 1.2 Add CLI arg parsing (e.g. `clap`) with `check <file>` and `run <file>` subcommands and a `--seed` option
- [ ] 1.3 Establish the pipeline skeleton: `lex -> parse -> typecheck -> (run | compile-grammar+decode)`
- [ ] 1.4 Add a `tests/` layout for golden output, negative type tests, and the litmus test; wire CI (cargo build + test)

## 2. Lexer and parser (language-grammar)

- [ ] 2.1 Define `Token`/`Span`; lex un-themed host keywords (`fn let var while if else print`) and occult keywords (`oracle summon divine enact fallback`, clause words `from using with confidence`)
- [ ] 2.2 Lex `spark`/`glyph` literals, identifiers, operators, punctuation, and `glyph` `${...}` interpolation; positioned lexical errors
- [ ] 2.3 Define AST nodes for programs, `fn`, statements, expressions, type declarations, and the `divine`/`enact` forms
- [ ] 2.4 Parse `type` declarations: records, `one_of` variants (with payloads), and `spark in a..b` refinements
- [ ] 2.5 Parse expressions with precedence, calls, method calls, variant construction, field access
- [ ] 2.6 Parse `divine ... from ... using ... with confidence >= ... fallback ...` and `enact <expr>`; positioned syntax errors for missing clauses
- [ ] 2.7 Document the formal grammar (EBNF) and keep it matching the parser

## 3. Host runtime (host-runtime)

- [ ] 3.1 Define the runtime `Value` model (spark, bool, glyph, record, variant, oracle, inferred, unit)
- [ ] 3.2 Implement scoped environments (lexical scopes, parent chain) with `let` immutability and `var` mutation
- [ ] 3.3 Evaluate literals, identifiers, records/variants, field access, and glyph interpolation
- [ ] 3.4 Evaluate arithmetic/comparison/boolean operators (short-circuit; division-by-zero error) and `if`/`while`/`print`
- [ ] 3.5 Evaluate `fn` declarations and calls (fresh scope, arity check, return value)
- [ ] 3.6 Unit tests for scoping, immutability, control flow, division-by-zero

## 4. Type system (type-system)

- [ ] 4.1 Define the `Type` representation: records, sum/variant types, `spark` refinements, `glyph`, `oracle`, and `Inferred<T>`
- [ ] 4.2 Implement the type checker for the host language (bindings, fn signatures, expressions, refinement-bound checks, variant validity)
- [ ] 4.3 Implement `Inferred<T>` as non-assignable to plain `T`; attach confidence + provenance to its representation
- [ ] 4.4 Implement the discharge rule: only `with confidence >= θ` narrows `Inferred<T>` to `T`; undischarged authoritative use is a type error
- [ ] 4.5 Implement `enact` exhaustiveness over variant action types (missing/extra variant = type error)
- [ ] 4.6 Ensure success diagnostics never claim semantic correctness (structural-only wording)
- [ ] 4.7 Negative type tests: out-of-range refinement, unknown variant, undischarged use, non-exhaustive enact

## 5. Constrained decoder (constrained-decoder)

- [ ] 5.1 Implement type -> grammar compilation (record -> ordered fields; one_of -> closed alternation; spark range -> bounded numeric; glyph -> bounded text)
- [ ] 5.2 Define the `Decoder` trait (generate value + confidence against a grammar) as the swappable backend seam
- [ ] 5.3 Implement the deterministic, grammar-respecting `MockDecoder` (seeded; honours grammar token-by-token; no network)
- [ ] 5.4 Derive a deterministic confidence from seed+grammar so both discharge paths (pass/fail) are exercisable in tests
- [ ] 5.5 Tests: determinism under fixed seed, illegal outputs unreachable, no network access

## 6. Oracle and divine (model-as-value, divine-inference)

- [ ] 6.1 Implement the `oracle` value and `summon` (binds model id + active decoder); type as `oracle`
- [ ] 6.2 Implement `divine`: evaluate inputs, compile output type to grammar, decode, build `Inferred<OutputType>` with confidence + provenance
- [ ] 6.3 Implement the `with confidence >= θ` discharge (success -> plain T) and `fallback` (failure branch); block undischarged downstream flow
- [ ] 6.4 Implement `enact` execution over discharged variant actions; thread provenance through to the enacted action
- [ ] 6.5 Ensure no untyped string escape hatch from oracle inference exists in the v0.1 surface
- [ ] 6.6 Tests: divine yields Inferred<T>, low-confidence takes fallback, sufficient-confidence yields T, provenance names oracle+seed

## 7. CLI and diagnostics (cli-toolchain)

- [ ] 7.1 Wire `witch check` (lex/parse/typecheck only; exit 0 iff well-typed; never execute)
- [ ] 7.2 Wire `witch run` (typecheck then execute; refuse to run ill-typed programs; stream `print` to stdout)
- [ ] 7.3 Wire `--seed` (and/or env var) into the decoder for reproducible `divine` runs
- [ ] 7.4 Render positioned, human-readable diagnostics for all error classes; never panic on user error; missing-file handling
- [ ] 7.5 CLI tests: hello run, check-passes/check-fails, type error blocks run, same-seed reproducibility, missing file

## 8. The litmus test and validation

- [ ] 8.1 Author the reduced §6.3 example program: `oracle` + `divine decision: Disposition` + discharge/fallback + `enact` (no memory/embedding/familiar)
- [ ] 8.2 Implement the litmus test: run the same program with the output type present vs. structurally removed under one seed; assert the generated output differs
- [ ] 8.3 Implement a fault-injection test (§6.2): force a low-confidence result and assert `fallback` fires and the value does not flow downstream
- [ ] 8.4 Add golden examples: hello/print, var loop, fn return, a passing typed `divine`, and the negative type-error cases
- [ ] 8.5 Run `openspec validate bootstrap-language-core --strict` and confirm every spec scenario is covered by a test
- [ ] 8.6 Write a short README: build, `witch check`/`witch run`, the `--seed` flag, and an explicit note that a green build is structural, not a correctness guarantee (§8)
