> Depends on `add-inference-runtime` (the `Engine` contract, `witch_ai_infer` ABI,
> `Engine::embed`, manifest + load-time resolution). Familiar and memory lowering have no
> model dependency and may land first; compiled `divine`/`embed` and the cross-engine
> proofs require change A.

> STATUS (in progress): Group 1 (familiar lowering) is **done and tested**. Groups
> 2–4 (native list/embedding/memory/within values + RC + ABI + lowering) and 5
> (compiled `divine` through the engine contract + manifest in the compiled
> runtime) are the remaining native-codegen work — large changes across `ir.rs`,
> `lower.rs`, `witchcraft-codegen`, and `witchcraft-runtime` (new heap payloads,
> RC, ABI symbols). Groups 7–8 (compiled litmus / engine-swap on a real local
> llama + frontier) additionally require `libllama` + a GGUF model and a live API
> key, which are unavailable in this offline environment, so the final acceptance
> bar cannot be executed here.

## 1. Familiar lowers as a function (smallest first)

- [x] 1.1 Remove the `Item::Familiar` lowering rejection; route a familiar through `lower_function`; calls dispatch via the existing function path <!-- lower.rs -->
- [x] 1.2 Permits + single-pass remain compile-time only (no runtime representation) — erased like `grant`
- [x] 1.3 Test: a familiar program (host + divine + enact) matches the interpreter under the same seed (Mock) <!-- codegen.rs familiar_lowers_like_a_function -->

## 2. Runtime values: list + embedding (RC heap)

- [ ] 2.1 Add `Value::List` and `Value::Embedding { space, vector, provenance }` to the compiled runtime as reference-counted heap payloads (reuse retain/release; immutable, acyclic)
- [ ] 2.2 ABI: list builder/iterator; `w_similarity`; `w_nearest` (returns a list value); `w_embed` (routes to `Engine::embed`; Mock = deterministic hash)
- [ ] 2.3 Factor one shared cosine + descending stable-sort/tie-break routine used by both interpreter and compiled runtime (or duplicate with a golden equality test)
- [ ] 2.4 Test: loop-local list/embedding values are reclaimed mid-run (no unbounded growth)

## 3. Runtime: governed memory registry

- [ ] 3.1 Add a compiled-runtime memory registry (thread-local): per-store entries, logical clock, retention filter, audit log — mirroring the interpreter
- [ ] 3.2 ABI: `w_mem_register`, `w_mem_write`, `w_mem_recent` (returns a list value), `w_advance`, `w_audit_log`
- [ ] 3.3 Test: registry state resets per run like seed/sink

## 4. Lowering rules (remove the rejections)

- [ ] 4.1 Lower `Expr::List`, `Expr::Method("embed")`, and the `similarity`/`nearest` builtins to the new ABI
- [ ] 4.2 Lower `Stmt::MemoryDecl` → `w_mem_register`; `Stmt::Within` → its body (scope erased); `mem.write`/`mem.recent`/`advance`/`audit_log` → ABI calls
- [ ] 4.3 Mirror the compile-time same-space restriction in the runtime arithmetic
- [ ] 4.4 Test: each previously-rejected construct now lowers and runs

## 5. Compiled divine through the engine contract

- [ ] 5.1 Emit the compiled `divine` decode call as `witch_ai_infer`, threading the evaluated `from (...)` input (stop evaluating-for-effect-and-discarding); embed the grammar; keep discharge/fallback/`enact` as compiled control flow
- [ ] 5.2 Compiled binary resolves needs against the manifest at load (locality vs `permit(network)`, litmus-strictness), exactly as the interpreter; refuse-to-start on no-match
- [ ] 5.3 Test: a compiled `divine` site passes its input to the engine and refuses a non-litmus-safe engine when litmus-strict

## 6. Equivalence (Mock byte-for-byte)

- [ ] 6.1 Extend `assert_compiled_equals_interpreted` to embedding, memory, and familiar example programs (Mock, same seed)
- [ ] 6.2 Make `examples/triage_flagship.witch` build with `grimoire build` and match `witch run` byte-for-byte (Mock); retire the separate compilable-only example special case
- [ ] 6.3 Test: the §6.2 runtime contrasts (low-confidence fallback, retention expiry, audit, out-of-scope/permit erasure) behave identically compiled vs interpreted

## 7. Compiled litmus (masking) — Verification B in compiled form

- [ ] 7.1 Run the falsification test against the NATIVE binary on each real litmus-safe engine: real grammar ⇒ in-grammar by construction; weakened ⇒ a token forbidden during generation; indistinguishable ⇒ fail loudly
- [ ] 7.2 Test: the compiler did not degrade constrained decoding to validate-after (masking demonstrated in the compiled path)

## 8. Compiled engine-swap (acceptance bar)

- [ ] 8.1 Build the flagship once with `grimoire build`; run the SAME binary against a real local llama model and against the frontier API, selected purely by manifest, with zero source change
- [ ] 8.2 Test: compiled output matches the interpreter per engine binding (compiled == interpreted across engines, not only Mock)

## 9. Validation

- [ ] 9.1 `cargo fmt --all`, `cargo clippy --workspace` clean
- [ ] 9.2 `cargo test --workspace` green (Mock-default fast/offline; real-engine + cross-engine tests gated like change A)
- [ ] 9.3 `openspec validate complete-native-compile --strict` clean; every spec scenario maps to a test
- [ ] 9.4 README: the interpreter and native compiler now accept the same language; `witch run` is purely the dev loop; the manifest-driven engine swap works for compiled binaries
