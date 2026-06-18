## 1. Lowering IR

- [x] 1.1 Define the IR data types (functions, blocks, instructions, typed values) as the backend-facing target
- [x] 1.2 Lower the type-checked AST → IR for the host language (literals, `let`/`var`, arithmetic/comparison/boolean, `if`/`while`, `print`, `fn` + calls)
- [x] 1.3 Lower records/variants, field access, and glyph interpolation into IR value ops
- [x] 1.4 Lower `divine`/discharge/`fallback` and `enact` into IR (decode call + branch + tag dispatch); never resolve inference at lower time
- [x] 1.5 Unit tests: representative programs produce expected IR; ill-typed programs never reach lowering

## 2. Runtime value model + reference counting

- [x] 2.1 Define the compiled runtime value representation: unboxed scalars; reference-counted heap payloads (glyph, record/variant fields, inferred inner+provenance)
- [x] 2.2 Implement retain/release/alloc behind a narrow runtime interface (so a later region fast-path can slot in)
- [x] 2.3 Emit retain/release at value moves and scope exits during codegen; free children on drop <!-- done for the host subset: LoadLocal retains, StoreLocal releases the old value, args transfer to the callee, locals released at scope exit, temporaries released after borrowing calls. Record/variant/inferred construction emission lands with group 4 -->
- [~] 2.4 Tests: loop-local values are reclaimed mid-run (no unbounded growth); no leaks on the examples (e.g. under a leak checker) <!-- compiled loop reclamation verified (live count returns to baseline). Records/variants/inferred now construct + release; enact-subject values (one per enact execution) are intentionally not yet released — acyclic, freed at exit — full leak-checker pass on the examples lands with the executable in group 5/6 -->

## 3. Cranelift backend

- [x] 3.1 Add the Cranelift dependency and a code generator from IR → Cranelift IR → object code <!-- backend is generic over cranelift_module::Module; JIT drives in-process execution now, object-module emission is wired in group 5 -->
- [x] 3.2 Implement calling convention and value passing for the runtime value representation <!-- the 16-byte #[repr(C)] value travels as two i64s (tag,bits); runtime extern "C" functions called/returned per the platform ABI -->
- [x] 3.3 Codegen host control flow + functions; link the runtime (value model, env-free compiled scopes) into the object <!-- scalars, glyphs+interpolation, arithmetic/comparison/equality, if/while, fn+calls, print; runtime linked via JIT symbols. Compiled output matches the interpreter on host.witch -->
- [~] 3.4 Emit the program entry point accepting program args and a `--seed` (argv/env per design open question) <!-- seed is threaded into the runtime for runs; the executable CLI entry (argv/--seed parsing) lands with `grimoire build` in group 5 -->

## 4. divine / oracle / enact in compiled form

- [~] 4.1 Serialise each `divine` output-type→grammar table into the artifact's data section <!-- grammars are compiled at build time and carried per `divine` site (converted to the runtime `Grammar`, variant tags interned to match `enact` dispatch). For JIT they are embedded as leaked pointer constants; the data-section serialisation format lands with object emission in group 5 -->
- [x] 4.2 Emit the runtime decode call (grammar handle → value + confidence + provenance) into the linked decoder; bundle the mock decoder <!-- `Decode` emits `w_divine` (grammar ptr + oracle/model + confidence out-param) into the runtime decoder; the deterministic mock decoder lives in `witchcraft-runtime::decode`, sharing one per-run RNG like the interpreter so multi-`divine` programs match -->
- [x] 4.3 Compile the `with confidence >= θ` discharge and `fallback` branch as native control flow; block undischarged downstream use (already a compile error from typeck) <!-- discharge lowers to a native branch on the decoded confidence; the fallback path returns/unwinds; undischarged `divine` emits `w_make_inferred`. Compiled fault-injection test confirms a forced-low confidence never reaches `enact` -->
- [x] 4.4 Compile `enact` to exhaustive variant-tag dispatch; thread provenance into the enacted action <!-- `VariantTag` + cranelift `Switch` dispatch over interned tags; arm bindings via `w_variant_field`; provenance threaded through `w_field` (propagated into the action) and bound to `provenance` in each arm via `ProvenanceGlyph`. Triage compiles to the interpreter golden -->
- [~] 4.5 Define the native oracle-adapter ABI so v0.2 backends attach behind the seam without codegen changes <!-- the seam is the `witchcraft-runtime::decode` entry (`w_divine` → grammar → value+confidence); v0.2 backends replace it with no codegen change. A formal pluggable adapter ABI/registration is deferred until a second backend exists -->

> Note: records/variants/field-access/inferred construction (IR ops carried since group 1, deferred in group 2.3/3) are now emitted here via the runtime builder + `w_field`/`w_variant_field`/`w_make_inferred`, completing host aggregate codegen alongside the AI-native core.

## 5. Linking and `grimoire build`

- [x] 5.1 Add the `grimoire` binary (or `witch build`) with `build <file> -o <out>` (typecheck → lower → codegen → link) <!-- new `grimoire` crate: `grimoire build <file> [-o out]` runs check → lower → `compile_object` (cranelift-object, with a C `main` entry parsing `--seed`) → link; also `grimoire check` and `grimoire --version` -->
- [~] 5.2 Bundle a linker (`lld`) and link the object + runtime into a single self-contained executable; system-linker fallback <!-- the object + runtime link into one self-contained native executable (verified Mach-O arm64; runs with no Rust/source). The runtime is embedded in `grimoire` (build.rs compiles a fresh dependency-free `staticlib` via rustc, no nested cargo) so the toolchain carries its own runtime. Linking currently drives the system `cc`; *bundling* `lld` to drop the system-linker requirement is a distribution refinement tracked for `add-distribution` -->
- [x] 5.3 `grimoire --version` reporting version + target triple (consistent with `witch`) <!-- `grimoire <version> (<target triple>)`, target captured in build.rs like `witch` -->
- [x] 5.4 Refuse to build ill-typed programs (no artifact, non-zero exit, structural-only success wording) <!-- `build` type-checks first; on diagnostics it prints them, exits non-zero, and writes no artifact (test: `ill_typed_program_is_refused_with_no_artifact`). Success wording keeps the structural-not-semantic caveat -->

> Note on macOS: cranelift-object emits a Mach-O object without a platform load command, which `ld` rejects (`unknown platform`); `compile_object` sets a `MachOBuildVersion` (PLATFORM_MACOS) before emit.

## 6. Equivalence and validation

- [x] 6.1 Conformance harness: for each example, assert `witch run --seed N` output == compiled-executable output for seed N <!-- `grimoire` tests build host.witch + triage.witch to native executables and compare their stdout to the interpreter across seeds (`*_executable_matches_interpreter`) -->
- [x] 6.2 Compiled litmus test: build with output type present vs structurally removed under one seed → outputs differ <!-- `compiled_litmus_deleting_the_type_changes_generation`. Object and JIT share one `build()`; the grammar is serialised into the artifact, so a built executable carries the type as a generation constraint -->
- [x] 6.3 Compiled fault-injection test: forced low confidence takes `fallback`, value does not flow downstream <!-- `compiled_fault_injection_keeps_low_confidence_out_of_enact`, matched against the interpreter -->
- [x] 6.4 Build the host + triage examples as executables in CI and run them; wire into the build/test workflow <!-- the `grimoire` integration tests build + run both example executables; CI runs `cargo test --workspace` and a dedicated `grimoire build`/run smoke step -->
- [x] 6.5 Run `openspec validate add-grimoire-codegen --strict` and confirm every spec scenario is covered by a test <!-- `openspec validate add-grimoire-codegen --strict` → valid; spec scenarios map to tests in lower.rs, codegen.rs, and grimoire/tests/build.rs -->
- [x] 6.6 Update README: `grimoire build` usage, the dev-loop vs ship-path split, and the structural-not-semantic caveat (§8)
