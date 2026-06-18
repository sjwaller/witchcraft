> Depends on `add-inference-runtime` (the `Engine` contract, `witch_ai_infer` ABI,
> `Engine::embed`, manifest + load-time resolution). Familiar and memory lowering have no
> model dependency and may land first; compiled `divine`/`embed` and the cross-engine
> proofs require change A.

> STATUS: Groups 1–7 are **done and tested offline** (Mock byte-equivalence,
> compiled litmus, governed memory, embeddings, lists, `within`, and the compiled
> `divine` routed through the *same* `Engine` contract + manifest the interpreter
> uses). The compiled engine-swap by manifest is proven on the native
> (Cranelift-JIT) path with the Mock binding — the SAME compiled flagship selects
> its model purely by manifest, byte-identical to `witch run`, and refuses to
> start on an unsatisfiable policy.
>
> HONEST BOUNDARIES (§8):
> - The shipped, self-contained `grimoire` executable stays **Mock-only**: its
>   runtime is a dependency-free `staticlib` built by bare `rustc` (no cargo
>   features), so it cannot pull `witchcraft`/`libllama`/`ureq`. Engine-swap is
>   carried on the JIT-compiled native path (full runtime linked with the
>   `engines` feature), not in the standalone binary. Wiring real engines into
>   the shipped artifact is a packaging follow-up (a cargo-built staticlib or
>   `--extern` rlib bundle), deliberately not attempted in this slice.
> - The real-engine LIVE acceptance (#3 against llama.cpp + a frontier API) was
>   **not exercised** here: no GGUF model and no API key are available. The
>   `llama` and `frontier` engine-swap tests are feature-gated, compile and run
>   (skipping, with a printed reason) under `--features llama` / `frontier`, and
>   are clippy-clean — but they have not been driven against real weights/keys.
> - Loose end 6.3 (real per-token logprob → confidence on the llama path) is
>   **left as the placeholder `1.0`**: extracting the chosen-token probability
>   from the current grammar+dist sampler risks the documented double-accept
>   grammar corruption and cannot be verified without a live model. Recorded, not
>   faked. The numeric-range single-token witness remains skipped.

## 1. Familiar lowers as a function (smallest first)

- [x] 1.1 Remove the `Item::Familiar` lowering rejection; route a familiar through `lower_function`; calls dispatch via the existing function path <!-- lower.rs -->
- [x] 1.2 Permits + single-pass remain compile-time only (no runtime representation) — erased like `grant`
- [x] 1.3 Test: a familiar program (host + divine + enact) matches the interpreter under the same seed (Mock) <!-- codegen.rs familiar_lowers_like_a_function -->

## 2. Runtime values: list + embedding (RC heap)

- [x] 2.1 Add `Value::List` and `Value::Embedding { space, vector, provenance }` to the compiled runtime as reference-counted heap payloads (reuse retain/release; immutable, acyclic) <!-- runtime/value.rs TAG_LIST/TAG_EMBEDDING, heap.rs release -->
- [x] 2.2 ABI: list builder/iterator; `w_similarity`; `w_nearest` (returns a list value); `w_embed` (deterministic hash, matching the interpreter's inline embed) <!-- runtime/abi.rs -->
- [x] 2.3 Factor one shared cosine + descending stable-sort/tie-break routine; duplicated into `runtime/embed.rs` with golden equality tests against the interpreter <!-- embedding_similarity_matches_interpreter, nearest_ranking_matches_interpreter -->
- [x] 2.4 Test: loop-local list/embedding values are reclaimed mid-run (no unbounded growth) <!-- loop_local_list_and_embedding_values_are_reclaimed -->

## 3. Runtime: governed memory registry

- [x] 3.1 Add a compiled-runtime memory registry (thread-local): per-store entries, logical clock, retention filter, audit log — mirroring the interpreter <!-- runtime/memory.rs -->
- [x] 3.2 ABI: `w_mem_register`, `w_mem_write`, `w_mem_recent` (returns a list value), `w_advance`, `w_audit_log` <!-- runtime/abi.rs -->
- [x] 3.3 Test: registry state resets per run like seed/sink <!-- sink::set_seed calls memory::reset; governed_memory_*/retention tests -->

## 4. Lowering rules (remove the rejections)

- [x] 4.1 Lower `Expr::List`, `Expr::Method("embed")`, and the `similarity`/`nearest` builtins to the new ABI <!-- lower.rs lower_call/lower_method, MakeList/Embed/Similarity/Nearest -->
- [x] 4.2 Lower `Stmt::MemoryDecl` → `w_mem_register`; `Stmt::Within` → its body (scope erased); `mem.write`/`mem.recent`/`advance`/`audit_log` → ABI calls <!-- lower.rs -->
- [x] 4.3 Mirror the compile-time same-space restriction: enforced by `typeck` (run before lowering), so cross-space comparison never reaches codegen <!-- cross_space_embedding_comparison_is_a_compile_error_on_the_native_path -->
- [x] 4.4 Test: each previously-rejected construct now lowers and runs <!-- list/embedding/memory/within equivalence tests -->

## 5. Compiled divine through the engine contract

- [x] 5.1 Emit the compiled `divine` decode (`w_divine`) threading the evaluated, rendered `from (...)` input to the engine (no more evaluate-for-effect-and-discard); the grammar rides in the artifact; discharge/fallback/`enact` stay compiled control flow <!-- ir.rs Decode{intent,input}, lower_divine_input, codegen Decode -->
- [x] 5.2 Compiled native path resolves needs against the manifest at load (locality vs `permit(network)`, litmus-strictness), exactly as the interpreter; refuse-to-start on no-match. Carried by `witchcraft-runtime`'s `engines` bridge + `Program::needs`. NOTE: active on the JIT path (full runtime, `engines` feature); the shipped `grimoire` staticlib is Mock-only by design (see STATUS) <!-- runtime/engines.rs, codegen bind_manifest -->
- [x] 5.3 Test: a compiled `divine` passes its input to the engine, swaps engine by manifest, and refuses to start under an unsatisfiable policy <!-- compiled_divine_through_mock_by_manifest_matches_interpreter, the_same_compiled_flagship_swaps_engine_purely_by_manifest, compiled_program_refuses_to_start_on_unsatisfiable_policy -->

## 6. Equivalence (Mock byte-for-byte)

- [x] 6.1 Compiled==interpreted extended to list, embedding, memory, `within`, and familiar programs (Mock, same seed) <!-- codegen.rs groups 2–4 tests -->
- [x] 6.2 `examples/triage_flagship.witch` builds with `grimoire build` and matches `witch run` byte-for-byte (Mock) <!-- grimoire/tests/build.rs flagship_executable_matches_interpreter, codegen flagship_compiles_and_matches_interpreter -->
- [x] 6.3 Test: the §6.2 runtime contrasts (low-confidence fallback, retention expiry, audit, out-of-scope/permit erasure) behave identically compiled vs interpreted <!-- compiled_fault_injection_keeps_low_confidence_out_of_enact, memory_retention_expiry_matches_interpreter, governed_memory_recency_and_audit_match_interpreter -->

## 7. Compiled litmus (masking) — Verification B in compiled form

- [x] 7.1 The native binary carries the output type as a generation constraint: with the type, generation is in-grammar by construction; weakened (type deleted) generation differs <!-- compiled_litmus_deleting_the_type_changes_generation -->
- [x] 7.2 Test: the compiler did not degrade constrained decoding to validate-after — on the Mock path masking is by construction (illegal outputs unreachable in the decoder). Token-trace masking against a real engine is the live llama acceptance, not exercised here (no model) <!-- decode.rs gen_value; engine falsify harness in change A -->

## 8. Compiled engine-swap (acceptance bar)

- [~] 8.1 The SAME compiled flagship runs against engines selected purely by manifest, zero source change — **proven on the native JIT path with the Mock binding** (laptop vs cloud manifests bind different models; provenance reflects the bind). Real llama + frontier are feature-gated and compile/run (skipping) but were NOT driven live (no GGUF/key); the shipped `grimoire` standalone binary remains Mock-only (see STATUS) <!-- the_same_compiled_flagship_swaps_engine_purely_by_manifest; compiled_flagship_runs_against_real_llama_by_manifest / compiled_divine_runs_against_real_frontier_by_manifest (feature-gated, env-skipped) -->
- [x] 8.2 Test: compiled output matches the interpreter per engine binding (compiled == interpreted under the same manifest, Mock) <!-- compiled_divine_through_mock_by_manifest_matches_interpreter -->

## 9. Validation

- [x] 9.1 `cargo fmt --all`, `cargo clippy --workspace` clean (default; and `--features llama` / `frontier` on codegen + witchcraft)
- [x] 9.2 `cargo test --workspace` green offline (Mock-default); real-engine tests feature-gated and env-skipped like change A
- [x] 9.3 `openspec validate complete-native-compile --strict` clean; every spec scenario maps to a test
- [x] 9.4 README: the interpreter and native compiler accept the same language; `witch run` is the dev loop; manifest-driven engine swap on the compiled (JIT) native path, with the honest Mock-only-standalone-binary boundary
