## 1. Lowering IR

- [x] 1.1 Define the IR data types (functions, blocks, instructions, typed values) as the backend-facing target
- [x] 1.2 Lower the type-checked AST → IR for the host language (literals, `let`/`var`, arithmetic/comparison/boolean, `if`/`while`, `print`, `fn` + calls)
- [x] 1.3 Lower records/variants, field access, and glyph interpolation into IR value ops
- [x] 1.4 Lower `divine`/discharge/`fallback` and `enact` into IR (decode call + branch + tag dispatch); never resolve inference at lower time
- [x] 1.5 Unit tests: representative programs produce expected IR; ill-typed programs never reach lowering

## 2. Runtime value model + reference counting

- [ ] 2.1 Define the compiled runtime value representation: unboxed scalars; reference-counted heap payloads (glyph, record/variant fields, inferred inner+provenance)
- [ ] 2.2 Implement retain/release/alloc behind a narrow runtime interface (so a later region fast-path can slot in)
- [ ] 2.3 Emit retain/release at value moves and scope exits during codegen; free children on drop
- [ ] 2.4 Tests: loop-local values are reclaimed mid-run (no unbounded growth); no leaks on the examples (e.g. under a leak checker)

## 3. Cranelift backend

- [ ] 3.1 Add the Cranelift dependency and a code generator from IR → Cranelift IR → object code
- [ ] 3.2 Implement calling convention and value passing for the runtime value representation
- [ ] 3.3 Codegen host control flow + functions; link the runtime (value model, env-free compiled scopes) into the object
- [ ] 3.4 Emit the program entry point accepting program args and a `--seed` (argv/env per design open question)

## 4. divine / oracle / enact in compiled form

- [ ] 4.1 Serialise each `divine` output-type→grammar table into the artifact's data section
- [ ] 4.2 Emit the runtime decode call (grammar handle → value + confidence + provenance) into the linked decoder; bundle the mock decoder
- [ ] 4.3 Compile the `with confidence >= θ` discharge and `fallback` branch as native control flow; block undischarged downstream use (already a compile error from typeck)
- [ ] 4.4 Compile `enact` to exhaustive variant-tag dispatch; thread provenance into the enacted action
- [ ] 4.5 Define the native oracle-adapter ABI so v0.2 backends attach behind the seam without codegen changes

## 5. Linking and `grimoire build`

- [ ] 5.1 Add the `grimoire` binary (or `witch build`) with `build <file> -o <out>` (typecheck → lower → codegen → link)
- [ ] 5.2 Bundle a linker (`lld`) and link the object + runtime into a single self-contained executable; system-linker fallback
- [ ] 5.3 `grimoire --version` reporting version + target triple (consistent with `witch`)
- [ ] 5.4 Refuse to build ill-typed programs (no artifact, non-zero exit, structural-only success wording)

## 6. Equivalence and validation

- [ ] 6.1 Conformance harness: for each example, assert `witch run --seed N` output == compiled-executable output for seed N
- [ ] 6.2 Compiled litmus test: build with output type present vs structurally removed under one seed → outputs differ
- [ ] 6.3 Compiled fault-injection test: forced low confidence takes `fallback`, value does not flow downstream
- [ ] 6.4 Build the host + triage examples as executables in CI and run them; wire into the build/test workflow
- [ ] 6.5 Run `openspec validate add-grimoire-codegen --strict` and confirm every spec scenario is covered by a test
- [ ] 6.6 Update README: `grimoire build` usage, the dev-loop vs ship-path split, and the structural-not-semantic caveat (§8)
