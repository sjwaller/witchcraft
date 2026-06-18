## Context

The Witchcraft paper (19-06-2025-witchcraft.md) is the source of truth. Its central claim is that "AI-native" is meaningful only when inference, memory, embeddings and agents are *primitives with their own types, semantics and runtime support* — and that the discriminator between a primitive and sugar is the litmus test (§6.3): **if you deleted the type, would the computation at the moment of inference change?** A prompt DSL fails this (the type is post-hoc validation); Witchcraft passes it (the type is compiled into a generation-time constraint).

An earlier draft of this change built the opposite: a string-returning `oracle` with the type system deferred and a fixed-fixture "recorded oracle." That is precisely the costume the paper rejects, and — critically — a fixed-fixture provider *cannot* pass the litmus test, because a fixture has no generation to change when you delete the type. This redesign corrects that. v0.1 exists to prove the thesis, deterministically.

This is the foundational change. Three seams defined here are inherited by everything later: the **inferred-value type** (a result carrying confidence + provenance), the **type→grammar compilation**, and the **decoder interface** (where real models attach in v0.2).

## Goals / Non-Goals

**Goals:**
- A `witch` CLI: `witch check` (type-check) and `witch run` (execute) a `.witch` file.
- A deterministic host language (un-themed plumbing): `fn`, `let`, `var`, `while`, `if`, `print`, expressions, lexical scope.
- A static type system with records, sum/variant types (`one_of`), and refinement types (`spark in 0..10`).
- The **inferred-value type** carrying confidence + provenance, and the **discharge rule** as a compile error.
- The **`divine`** construct and **`enact`** with exhaustiveness checking.
- A **constrained decoder**: output type → generation grammar → deterministic grammar-respecting reference decoder.
- Tests that prove it: a **litmus test** (delete the type → generation changes) and **negative type tests** (undischarged use / non-exhaustive enact → compile errors).

**Non-Goals:**
- `memory`, `embedding`, `familiar` primitives — each is its own §5 primitive; separate changes.
- Live model backends (Ollama/llama.cpp/API) — v0.2, behind the same decoder interface.
- Compiled/WASM/native backend; performance work.
- **Semantic correctness** of model outputs — the compiler checks structure, not truth (§8). A green build is explicitly *not* a correctness guarantee.
- Grimoire build tool; Coven packages; full §6.3 example (needs memory/embedding/familiar).

## Decisions

### D1: Rust workspace, library + thin binary
`witchcraft` library (lexer, parser, types, host interpreter, decoder, oracle) + a thin `witch` binary. **Why:** strong enums for AST/types/values, excellent test story, reusable by a future compiler front-end. The lexer/parser/type-checker are *not* throwaway when a compiled backend later replaces the interpreter.

### D2: The type system is the deliverable, not a deferral
Unlike the prior draft, static typing is in scope from line one, because per the paper the type system *is* the nativeness. Pipeline: `lex → parse → typecheck → (run | emit decoder grammar)`. **Why:** deferring types would ship sugar and disprove the thesis.

### D3: The inferred-value type carries confidence + provenance
Inference does not return a bare value. `divine`/oracle inference yields `Inferred<T>` — a value wrapping a `T` together with a confidence scalar and a provenance record (which oracle, which inputs, prompt lineage). **Why (§5.1, §6.3):** this is what lets the compiler distinguish an inferred value from an ordinary one and enforce that it is discharged before authoritative use. *Alternative:* return bare `T` and track confidence on the side — rejected, it's exactly the opaque-client move the paper rejects.

### D4: The discharge rule is a compile-time check
An `Inferred<T>` cannot be used where a plain `T` is required. It is discharged only via a confidence gate (`with confidence >= θ`), which on success narrows it to `T` and on failure takes the `fallback` branch. Using an undischarged `Inferred<T>` authoritatively is a **type error**. **Why (§4.2, §6.3):** this is the headline "runtime error converted to compile error" — the concrete, checkable guarantee of nativeness.

### D5: `divine` — output type as specification
```
divine decision: Disposition
  from (msg, history)
  using triage
  with confidence >= 0.80
  fallback <expr>
```
The declared output type (`Disposition`) is compiled to a generation grammar; the oracle generates *into* that grammar; the result is `Inferred<Disposition>` discharged by the confidence clause. `enact` then executes the typed action, exhaustively over its variants. **Why:** this is the paper's keystone — inference *is* the computation, bounded by the type, not a value fetched by hand-written control flow.

### D6: Type → grammar compilation
Each output type compiles to a formal generation grammar: a record → an ordered field grammar; `one_of { A, B, C }` → an alternation over exactly those variants; `spark in 0..10` → a numeric range constraint; `glyph` → a (bounded) text production. **Why:** the grammar is the mechanism that makes the type "part of the computation" rather than a post-hoc check. This compiler is the heart of the project.

### D7: The decoder is deterministic AND grammar-respecting (the unlock)
v0.1 ships a `Decoder` trait and a single `MockDecoder`: it samples **deterministically** from a seed, but is **constrained token-by-token by the compiled grammar** so it can only emit values inside the type. **Why this is the crux:** a fixed-fixture fake cannot pass the litmus test (no generation to change); a free LLM is non-deterministic (no test loop). A seeded, grammar-respecting fake gives *both* — deleting the type removes the grammar, so generation genuinely changes, while a fixed seed keeps runs reproducible. Real backends (Ollama/llama.cpp) implement the same `Decoder` trait in v0.2. *Alternatives rejected:* recorded fixtures (fails litmus), live model now (non-deterministic, premature).

### D8: Provenance flows downstream structurally
The provenance attached to an `Inferred<T>` survives discharge and rides into `enact`, so the audit trail is structural, not hand-logged. **Why (§6.3, §8):** the human who declared the type must be able to answer for what `enact` does; provenance is for that reader.

### D9: Un-theme the plumbing (§7)
Mundane constructs use plain names: `fn`, `let`, `var`, `while`, `if`, `print`. Occult vocabulary is reserved for the genuinely new: `oracle`, `summon`, `divine`, `enact`, `fallback` (and later `memory`, `embedding`, `familiar`, `permits`). Type names `spark`/`glyph` follow the paper's §6.3 example. **Why:** §7 — theming the familiar subtracts recognition and dresses non-determinism as magic; keep the name, let the plumbing look like plumbing.

### D10: Tests encode the paper's discriminator
Beyond golden output tests, the suite includes: (a) the **litmus test** — the same `divine` program type-checked/run with the output type present vs. structurally weakened, asserting the generated value differs; (b) **negative type tests** — undischarged use and non-exhaustive `enact` must fail `witch check`; (c) a **fault-injection** sketch (§6.2) — a low-confidence result must take `fallback`, not flow through. **Why:** makes the nativeness claim falsifiable, per the paper's own demand.

## Risks / Trade-offs

- **Mock decoder gives false confidence about real models** → Scoped explicitly: v0.1 proves the *language machinery* (constraint, discharge, provenance), not model quality. §8's "structural ≠ semantic" is stated in specs and surfaced in CLI docs. v0.2 adds real backends + their own integration tests.
- **Type→grammar compilation is the hard part and easy to under-build** → Keep v0.1 grammar coverage minimal but real (records, finite variants, integer ranges, bounded text). Richer types are additive later.
- **Single `oracle` type may be too coarse (§5.5 self-critique)** → Acknowledged. v0.1 keeps `oracle` as one type but isolates it behind the `Decoder`/effect seam so a future *family* of model effects can refine it without reshaping callers.
- **Mistaking the green build for correctness (§8, §10.1)** → The most dangerous misread. Mitigated by spec requirements stating the boundary explicitly and by CLI/docs language; not a thing code can prevent, so it's named loudly.
- **Scope creep toward the full §6.3 example** → memory/embedding/familiar are firmly out; v0.1 targets the reduced oracle+divine+enact example. Each deferred primitive is its own change, preserving coherence (the OpenSpec discipline).
- **Interpreter-first vs "compiled first" principle** → Conscious trade-off; the front-end (lex/parse/typecheck/grammar) is shared with a future compiler, so it is not wasted.

## Open Questions

- Confidence model for the mock decoder: how is a deterministic "confidence" derived for a generated value (e.g. from constraint slack / seed) so the discharge gate is exercisable in tests? (Leaning: a deterministic function of the seed + grammar so both pass and fail paths are testable.)
- Refinement-type breadth for v0.1: integer ranges only, or also string patterns / length bounds? (Leaning: integer ranges + finite variants + bounded text; defer regex-style refinements.)
- Provenance representation: structured record vs opaque token. (Leaning: a small structured record — oracle id, input digest, seed.)
- `divine` inputs: typed tuple `from (a, b)` only, or arbitrary expressions? (Leaning: typed tuple of in-scope values for v0.1.)
