## Why

The Witchcraft paper argues that "AI-native" means almost nothing in current practice: it usually denotes themed keywords over an inference SDK — a model call that returns a string, indistinguishable to the compiler from any other side effect. That is sugar. The paper's thesis is that **nativeness lives in the type system**: a construct is a genuine primitive only if its type is treated specially, it makes new errors statically checkable, and the runtime is built around it (§4). The decisive test (§6.3) is the *litmus test*: if you deleted the type, would the computation at the moment of inference change? For a prompt DSL, no. For Witchcraft, yes — because the output type is compiled into a generation-time constraint, not validated after the fact.

This change bootstraps the smallest core that **passes that litmus test deterministically**, rather than the smallest core that merely runs. Proving the thesis up front is what de-risks the whole project; building string-returning sugar would disprove it.

## What Changes

- A `witch` CLI with `run` (execute) and `check` (type-check only) over `.witch` source.
- A small **deterministic host language** (the "plumbing", deliberately un-themed per §7): `fn`, `let` (immutable), `var` (mutable), `while`, `if`, `print`, expressions, lexical scope. Type names `spark` (numeric) and `glyph` (text) follow the paper's canonical example.
- A **static type system** that makes the paper's structural guarantees real:
  - record types, sum/variant types (`one_of { ... }`), and refinement types (`spark in 0..10`).
  - an **inferred value** type that carries *confidence* and *provenance* as part of the value.
  - the **discharge rule**: using an inferred value authoritatively without a confidence gate is a **compile error**.
  - **exhaustiveness**: `enact` over a variant action type must cover exactly the declared variants.
- The keystone construct **`divine`** (§6.3): a typed inference region whose declared output type *is* the specification, with `from` (inputs), `using` (oracle), `with confidence >= θ` (discharge), and `fallback`.
- **`oracle`** as a first-class typed value (`summon`), whose `.invoke`/inference is typed as an effect producing an inferred value.
- A **constrained decoder** runtime: the output type is compiled into a generation grammar, and a **deterministic, grammar-respecting reference decoder** (seeded) produces values token-by-token within that grammar. Deleting the type removes the constraint and changes the generated output — so the litmus test passes **and** stays deterministic for tests. Real model backends slot behind the same interface later.
- A golden + property test harness, including an explicit **litmus test** (same program with/without the output type → observably different generation) and **negative type tests** (undischarged use, non-exhaustive `enact` → compile errors).

**Non-goals (deferred to later changes):** the `memory`, `embedding`, and `familiar` primitives (each is its own §5 primitive and gets its own change); live model backends (Ollama/llama.cpp/API — v0.2, same decoder interface); compiled/WASM/native backend; semantic correctness guarantees (the compiler verifies structure, not truth — §8); Grimoire build tool; Coven packages. The flagship full §6.3 triage example (which also uses memory/embedding/familiar) is the post-v0.1 integration milestone; v0.1 targets a reduced version using `oracle` + `divine` + `enact`.

## Capabilities

### New Capabilities
- `language-grammar`: surface syntax — un-themed host constructs, type declarations (records, `one_of` variants, refinements), and the `divine`/`enact` clause grammar.
- `host-runtime`: the deterministic host-language evaluation — values, scoping, `fn`/`let`/`var`/`while`/`if`/`print`, arithmetic/comparison/boolean operators. The plumbing the primitives compose with.
- `type-system`: the static checks that constitute nativeness — records/variants/refinements, the inferred-value type carrying confidence + provenance, the discharge rule, and `enact` exhaustiveness. Compile errors, not runtime checks.
- `model-as-value`: `oracle` as a typed first-class value (`summon`), inference typed as an effect returning an inferred value, provenance origin.
- `divine-inference`: the `divine` block — output type as specification, confidence discharge + `fallback`, provenance threading, and the litmus property.
- `constrained-decoder`: the decoder runtime — compiling an output type into a generation grammar, the deterministic grammar-respecting reference decoder, and the unreachability-of-illegal-outputs guarantee.
- `cli-toolchain`: `witch run` / `witch check`, decoder seed configuration, and human-readable structural diagnostics.

### Modified Capabilities
<!-- None — first change; no existing specs. -->

## Impact

- New Rust workspace: `witchcraft` library (lexer, parser, type checker, host interpreter, decoder, oracle) + thin `witch` binary.
- Establishes the load-bearing seams the whole project inherits: the **inferred-value type** (confidence + provenance), the **type→grammar compilation**, and the **decoder interface** (where real models later attach). Coherence here matters disproportionately.
- New developer dependency on the Rust toolchain (`cargo`).
- Test suite includes property/litmus tests, not just golden output — the acceptance bar is the paper's fault-injection discipline (§6.2): the native construct must fail more safely than a library equivalent, or it is decoration.
