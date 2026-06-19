## Why

The config's standing decision is that "Witchcraft is a COMPILED language; the tree-walking interpreter is the bootstrap and dev loop, not the end state," and `grimoire-codegen` established compiled/interpreted *equivalence* as a discipline. But the three later primitives — embedding, governed memory, and the bounded familiar — were shipped interpreter-only: `crates/witchcraft/src/lower.rs` explicitly rejects list literals, `oracle.embed`, the `similarity`/`nearest` builtins, `memory` declarations, `within`, and `familiar` items ("not supported by the compiler yet (use `witch run`)"). The §6.3 flagship therefore only runs under `witch run`, which undercuts the compiled-language claim and leaves the equivalence discipline incomplete. This change closes the Cranelift gap so memory, embedding, and familiar lower to the native `grimoire build` path and produce **identical** output to the interpreter under the same seed.

## What Changes

For each construct, replace the lowering rejection with a real lowering rule plus the runtime/ABI it needs (all pure-Rust, linked like the existing compiled runtime — no new language):

- **Embedding.** Add `Value::Embedding { space, vector, provenance }` and `Value::List` to the compiled runtime; ABI `w_embed` (routes to the inference runtime's `Engine::embed`; the `Mock` engine's deterministic hash is the test path), `w_similarity`, `w_nearest`, and a list builder/iterator. Lower `Expr::List`, `Expr::Method("embed")`, and the `similarity`/`nearest` builtins. The cross-space compile error is already enforced in the type checker; the runtime mirrors the interpreter's same-space guard.
- **Governed memory.** Add a compiled-runtime memory store (a global registry with a logical clock, retention filtering, and an audit log) and ABI `w_mem_register`, `w_mem_write`, `w_mem_recent`, `w_advance`, `w_audit_log`. Lower `Stmt::MemoryDecl` to a registration call, `Stmt::Within` to its body (scope is compile-time-erased, as in the interpreter), and `mem.write`/`mem.recent` to the ABI. Scope enforcement stays a compile-time check.
- **Familiar.** Lower `Item::Familiar` exactly like a function: permits and the single-pass/bounded rule are compile-time properties already enforced by the checker, so at runtime a familiar is an ordinary single-pass call. This is the smallest of the three — remove the item-loop rejection and reuse `lower_function`.
- **Equivalence + flagship.** Extend the existing compiled/interpreted equivalence tests to cover embedding, memory, and familiar, and make `examples/triage_flagship.witch` build with `grimoire build` and match `witch run` byte-for-byte under the same seed (`Mock` engine). The separate compilable `examples/triage.witch` need no longer be a special case. The §6.2 fault-injection contrasts must hold identically in compiled form — the four "will not compile" cases are already compile-time (`grimoire build` refuses ill-typed programs), and the runtime ones (low-confidence fallback, retention expiry, audit, out-of-scope/permit erasure) must behave the same compiled as interpreted. This change adds **no new primitive**, so it inherits the §6.2 bar rather than needing to clear it afresh.
- **The litmus and engine-swap hold *compiled*, not just interpreted.** `add-inference-runtime` proves the falsification test and the cross-engine (local + frontier) flagship via the interpreter (`witch run`). This change must show the **same two proofs in the compiled binary**: (a) the falsification/litmus test passes for the native artifact against the real engine(s); (b) a `grimoire build` binary of the flagship runs against a real **local** model and the **frontier** API, selected purely by manifest, with **no source change** — and its output equals the interpreter's per engine. This is the real meaning of "compiled == interpreted" for an AI-first language: equivalence across *engines*, not only under the `Mock`.

**Non-goals (deferred):** authoring new engines (that is `add-inference-runtime`; this change only routes the compiled path through the existing `Engine` contract); durable/external memory storage; semantic memory retrieval (still composed at the flagship layer); any new language feature — this change only *lowers* constructs that already exist and type-check.

## Capabilities

### Modified Capabilities
- `grimoire-codegen`: extend the lowering/codegen coverage from "host language + `divine`/`enact`" to also include embeddings, governed memory, and familiars, preserving the compiled/interpreted equivalence requirement (now asserted for these constructs and the flagship).

## Impact

- Builds on (archived): `add-grimoire-codegen` (IR, Cranelift backend, RC runtime, ABI), `add-embedding-primitive`, `add-memory-primitive`, `add-familiar-primitive` (the constructs to lower), and `integrate-triage-flagship` (the equivalence target).
- **Depends on `add-inference-runtime`**: the engine contract + ABI seam (`witch_ai_infer`, `Engine::infer`/`Engine::embed`, the manifest/resolution) must exist first so the compiled path can call it and so the compiled litmus/engine-swap proofs (above) have real engines to run against. Memory and familiar do not need a model and could land independently, but are kept in this change for one coherent "the compiler now covers the whole language" step.
- After this change, the interpreter and the native compiler accept the same language; `witch run` is purely the dev loop.

## Open Questions (leans in design.md)

1. Memory store identity/lifetime in a compiled artifact (global static registry vs passed handle). **Lean: a global thread-local registry mirroring the interpreter.**
2. `Value::List` / `Value::Embedding` representation within the 16-byte `repr(C)` value. **Lean: reference-counted heap payloads, like glyph/record.**
3. Does compiled `embed` require a real engine, or does the `Mock` hash suffice for equivalence? **Lean: `Mock` engine for equivalence tests; real `embed` is opt-in via `add-inference-runtime`'s manifest/policy.**
4. Sort/tie-break and float determinism for compiled `nearest`/`similarity` vs the interpreter. **Lean: share one arithmetic + tie-break routine so both paths are bit-identical under the `Mock` engine.**
