## Why

Three gaps stand between the current artifact and the paper's thesis. In order of severity:

- **BREAK 1 — the make-or-break: the litmus is unproven against a real model.** The entire AI-first claim (§6.3) rests on grammar-constrained decoding **enforced during generation** — token-by-token logit-mask / GBNF — so illegal outputs are *unreachable*, not validated-after. Today this holds only against the deterministic mock (`crates/witchcraft-runtime/src/decode.rs`), which respects the grammar because it was built to. The thesis is real **only if** a real decoder against a real tokenizer holds the same property. If real tokenisation forces a degrade to generate-then-validate-then-resample, the litmus fails and Witchcraft is a wrapper. The headline deliverable of this change is a **falsification test** that proves-or-breaks this, per engine.
- **BREAK 3 — no real inference.** `divine` calls a seeded PRNG that ignores the prompt (the compiled `w_divine` ABI has no input parameter; codegen evaluates the `from (...)` inputs "for effect" and drops them). Replace the mock as the *execution path* with real engines.
- **BREAK 2 — native compilation incomplete.** memory/embedding/familiar run only under the `witch run` interpreter, not the `grimoire build` Cranelift path (§8/§13 frame Witchcraft as a *compiled* AI-first language). **This break is owned by the sibling change `complete-native-compile`**; it is stated here so the dependency is explicit.

This change (`add-inference-runtime`) owns Break 1 and Break 3: it defines a **pluggable inference contract** the application is written *against*, ships two real engines plus the demoted mock, and makes the litmus a permanent, per-engine canary. §8 honesty stays loud throughout: the contract guarantees **shape and policy**, never **quality**. "AI-first" means the language is native to inference — not that the model is good.

**Explicitly NOT a break (so it is not elevated):** capability/quality *tiers* — "is the model smart enough to do this well?" Grammar guarantees shape, never quality; §8 concedes this as a *limit of the thesis*, not a failure of it. A `tier` is shipped as **optional/advisory only** and a precise tier metric is named as an explicit out-of-scope open question. It must block nothing.

## What Changes

### The headline: a falsification test that would disprove AI-first
The first thing this change designs is the test that protects the thesis forever:
- Take a `divine` site with a constrained output type. Build it **twice** — once with the real type's grammar, once with the type **weakened** (the existing weakened-grammar lowering) — and run both through the **real** decoder.
- **Assert:** with the real grammar, every generated token is in-grammar and the output inhabits the type *by construction*; with the weakened grammar, generation genuinely differs (out-of-grammar tokens become reachable). If the two are indistinguishable, the litmus has **failed** — the test fails loudly and says the language is a wrapper.
- It runs against **each real engine**, not just the mock. It is the §6.2 fault-injection bar for this change.

### The pluggable inference contract (the interface)
Inference becomes a **swappable engine** the application is written *against*, never a model it is bound to — so an app written today gets smarter as better engines are plugged in, with **no source change**. Four layers; only the first two ever appear in application source:

1. **NEED (source-visible):** input, typed output shape (→ generation grammar), and a stable `intent_id` naming what the inference is *for*. Source names a NEED (the `oracle` need-handle + output type), **never** a model/vendor/engine. A model name in application source is a structural design violation.
2. **POLICY (source-visible, §9.1):** author-stated *constraints* — on-device-only vs network-allowed (`permit(network)`, reusing capability-effects), latency class, optional minimum tier (advisory). Constraints, not engine choices.
3. **BINDING (manifest/deployment, NOT source):** how a need resolves to a concrete engine in *this* deployment. Same program + different manifest = laptop / edge / GPU / cloud. Selection happens at **load**, matched on need + policy.
4. **ENGINE TRAIT (implementer-facing):** the contract every engine implements — `describe()` (tiers, modalities, LOCAL vs NETWORK, latency class, and **mandatory** whether it supports grammar-constrained token-by-token decoding) and `infer({intent_id, input, output_grammar, policy, seed?}) -> {value, confidence, provenance}` where `value` inhabits `output_grammar` **by construction**.

### The engines (prove the contract with more than one)
- **LOCAL — llama.cpp via FFI.** GBNF grammar support is built in (the safest way to prove Break 1). Links `libllama` (C/C++) **in CI only**; the language/toolchain stays self-contained, only model *execution* links a lib; the GGUF artifact sits beside the binary.
- **NETWORK — a frontier API engine** (Anthropic/OpenAI-style). It must honour the grammar; whether provider structured-output counts as grammar-by-construction is decided **empirically by the falsification test** — if it passes it is litmus-safe, if it only validates-after it is marked **non-litmus-safe** with reasons.
- **TEST — the deterministic mock, demoted.** Kept only as a selectable test engine implementing the same trait, so litmus/equivalence tests stay deterministic while real runs use real engines. Never deleted.

### Hard invariants
- The language trusts the **contract**, not the engine. Grammar-by-construction is the universal property every engine — present or future — must satisfy; an engine that cannot honour the grammar *during* generation is **not a legal engine**.
- If no engine satisfies the **policy** (e.g. on-device-only on a box with no local engine), the program **refuses to start** — it never silently violates policy. (Tier is advisory and never causes refusal.)
- Confidence and provenance come from the engine, never synthesised; provenance records `{ model_id, model_version_or_sha, backend_id, seed, sampling }` so §6.2's "a model version change is flagged by provenance" actually holds.

**Non-goals (deferred):** a precise tier *metric* (advisory string only here); training/fine-tuning; GPU/NPU kernels (the engine *selects* a device); resolving §5.5's `oracle`-granularity question (the contract is shaped not to foreclose splitting `oracle` later, but v0.2 keeps one type); native lowering of memory/embedding/familiar (that is Break 2 / `complete-native-compile`, though `Engine::embed` is defined here for it to consume). This change adds a real **execution path**; it cannot make a model's output correct (§8).

## Capabilities

### New Capabilities
- `inference-runtime`: the NEED/POLICY/BINDING/ENGINE contract, the manifest + load-time resolution (refuse-to-start on no-match), the falsification test, two real engines + the demoted mock, real confidence/provenance, and the `permit(network)` policy gate.

### Modified Capabilities
- `constrained-decoder`: the decoder interface becomes the engine `infer` contract carrying `intent_id` + input + policy; grammar-by-construction during generation is mandatory; the deterministic decoder is reframed as the `Mock` test engine (still the offline default and the litmus oracle in tests).
- `model-as-value`: the `oracle`'s string is a **semantic intent id resolved by manifest**, not a model name; provenance gains `model_version_or_sha` and `backend_id`.
- `divine-inference`: input is threaded to the engine; `divine` is litmus-strict by default and refuses a non-litmus-safe engine absent an explicit downgrade; the litmus must hold against real engines.

(The compiled-codegen wiring of the `witch_ai_infer` ABI is owned by the sibling change `complete-native-compile`, which modifies `grimoire-codegen`; this change defines the contract + ABI shape and proves it via the interpreter path.)

## Impact

- Builds on (archived): `bootstrap-language-core` (divine/decoder seam), `add-grimoire-codegen` (the `w_divine` ABI to evolve), `add-capability-effects` (reused for `permit(network)`), `add-distribution` (offline-mock-default + "real backends optional" contract this must honour).
- **Required by `complete-native-compile`**: the engine `infer`/`embed` ABI must exist before the AI primitives can lower to native code.
- No change to existing programs' behaviour: the `Mock` engine remains the default for offline first-run/tests/distribution, so every current example stays deterministic and offline.

## Locked decisions (approved)

1. **First local engine: llama.cpp via FFI** (GBNF gives battle-tested token-by-token constraint; Break 1 is the one place we do not hand-author the critical mechanism; C++ is confined to model execution, toolchain stays self-contained; candle may be added later behind the same trait).
2. **`oracle` string = semantic intent id** (e.g. `"TriageReasoner"`), never opaque, never a model/vendor; a model name in source is a structural design violation; spec records this as a deliberate sharpening of §5.1.
3. **(Verification A) Non-litmus-safe engines are first-class; a `divine` site is litmus-strict by default and refuses a non-litmus-safe engine at load** (like a locality no-match). Running on one requires an explicit, source-visible downgrade acknowledgement — validate-after can never sneak back in silently.
4. **(Verification B) The falsification test asserts masking occurred** — at ≥1 decode step a token the weakened grammar permits is forbidden by the real grammar — not an output comparison; fails loudly if masking cannot be shown, per engine.
5. **(Verification C) `--seed` honesty is user-visible** — docs/CLI state the seed is a determinism contract only with the `Mock` engine and best-effort with real engines.

## Remaining open questions (leans in design.md)

1. Which Rust FFI crate for llama.cpp + which GGUF model in CI. **Lean: `llama-cpp-2`; a small instruct GGUF fetched out-of-band.**
2. Manifest format + resolution + no-match behaviour. **Lean: small TOML `intent_id → engine + model + locality/latency/tier`, resolved once at load; no-match = refuse to start.**
3. Frontier grammar fidelity. **Lean: decided empirically by the falsification test, not by fiat; map `Grammar`→JSON-Schema; mark non-litmus-safe if it cannot demonstrate masking.**
4. Tier metric. **Lean: out of scope; advisory engine-declared string only; must block nothing.**
