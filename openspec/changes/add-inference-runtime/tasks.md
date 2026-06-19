## 1. The Engine contract

- [x] 1.1 Define `Engine` trait (`describe`, `infer`, `embed`) with `EngineDescription`, `InferRequest { intent_id, input, output_grammar, policy, seed }`, `InferResult { value, confidence, provenance }`, `Policy`, and `Provenance { model, model_version_or_sha, backend_id, seed, sampling }` <!-- crates/witchcraft/src/engine.rs -->
- [x] 1.2 Engine resolution/registry: build engines per binding, refusing unknown kinds and non-litmus-safe bindings <!-- manifest::build_engine -->
- [x] 1.3 Collapse the interpreter `Decoder` seam onto the `Engine` contract (callers use the bound engine, not a hard-coded decoder) <!-- interp exec_divine routes through Engine -->
- [ ] 1.4 Unit tests: a grammar-incapable engine (`grammar_constrained = false`) is rejected for a `divine` need

## 2. Mock engine (demoted, retained)

- [x] 2.1 Reimplement the deterministic grammar-respecting decoder as `kind = "mock"` behind the `Engine` trait (deterministic per seed, token-by-token, no network) <!-- engine/mock.rs MockEngine -->
- [x] 2.2 Make Mock the default when no manifest/model is present (offline first-run, examples, CI) <!-- interp default_engine -->
- [x] 2.3 Tests: Mock is the deterministic litmus oracle; existing examples stay byte-identical and offline <!-- workspace goldens unchanged; falsification.rs -->

## 3. ABI evolution (carry the input)

- [ ] 3.1 Define the versioned C ABI `witch_ai_infer(const WitchInferRequest*, WitchInferResult*)` (the C ABI lands with the compiled wiring in `complete-native-compile`; the Rust `InferRequest`/`InferResult` shape is defined here)
- [x] 3.2 Thread `divine` inputs to the engine (interpreter path): inputs are evaluated into the prompt and passed in `InferRequest.input` (no longer dropped) <!-- interp exec_divine -->
- [ ] 3.3 Tests: the resolved `from (...)` input is observable in the engine request

## 4. Source surface: NEED + POLICY

- [x] 4.1 Reinterpret the `oracle ... = summon "<intent-id>"` string as a semantic intent id; provenance/manifest match on it (§5.1-sharpening) <!-- interp intent; lower.rs; provenance.oracle=intent -->
- [x] 4.2 `divine` is litmus-strict by default; a source-visible downgrade (`permit(unsafe_inference)`) permits running a strict need on a non-litmus-safe engine <!-- Policy.litmus_strict/allow_downgrade; oracle_policies -->
- [x] 4.3 Reuse `permit(network)` (capability-effects) as the locality policy: absence ⇒ on-device-only; presence ⇒ network engine eligible <!-- oracle_policies; manifest resolve -->
- [ ] 4.4 Optional `with policy { latency <class>, tier >= <t> }` on the summon (latency hard hint; tier advisory-only) — surface deferred; manifest carries latency/tier today
- [x] 4.5 Tests: `permit(network)` legibly gates network eligibility (refuse-to-start without it) <!-- tests/manifest.rs -->

## 5. Manifest + load-time resolution

- [x] 5.1 TOML manifest: `[need.<intent>]` → engine + model + sha256 + locality + latency + tier; `[engine.<id>]` → kind + params; models named ONLY here; credentials from env <!-- manifest.rs -->
- [x] 5.2 Resolve every used need at load against the manifest + program policy (locality vs `permit(network)`, litmus-strictness) <!-- interp resolve_engines; manifest resolve -->
- [x] 5.3 Refuse to start on no-match with a diagnostic naming the need + unmet constraint; never silently cross a policy boundary <!-- ResolveError -->
- [x] 5.4 Tests: manifest binds intent → engine; on-device-only refuses a network binding; unknown need refuses <!-- tests/manifest.rs -->

## 6. Local engine — llama.cpp via FFI (Break 1 target)

- [x] 6.1 Add the llama.cpp FFI dependency (link `libllama`, confined to model execution; CI builds it; default tests stay on Mock) <!-- llama-cpp-2 0.1.150 behind --features llama; cmake + C++ toolchain build verified locally -->
- [x] 6.2 Map our `Grammar` → GBNF (variants → alternation, refined int → bounded numeric, glyph → bounded text, records → ordered fields); report unmappable features as "cannot serve this need" (refuse, never downgrade) <!-- engine::grammar_to_gbnf; exercised by real llama sampler -->
- [x] 6.3 Implement `infer` with GBNF token-by-token constraint; fill provenance (`model_id`, `model_version_or_sha = manifest sha256`, `backend_id`, seed, sampling) and a real confidence from the sampler <!-- llama.rs generate(): real GBNF chain (grammar mask + dist) drives token-by-token decode; provenance filled. Confidence now the geometric mean of the chosen-token masked probability, read via a lockstep probe grammar sampler (no double-accept); live witness 0.568 in-type vs 0.044 weakened -->
- [x] 6.4 Implement `embed` behind the engine (Mock reuses the deterministic embedding; default trait method) <!-- engine embed; mock embed_hash -->
- [x] 6.5 Tests: real local `divine` of a constrained type yields an in-type value by construction (libllama + real GGUF) <!-- tests/falsification.rs::real_llama_masks_tokens_during_generation, WITCHCRAFT_GGUF -->

> Group 6 status: **VERIFIED against real llama.cpp weights** (Qwen2.5-0.5B-Instruct
> Q4_K_M, arm64 Metal). The make-or-break litmus HOLDS: the falsification harness,
> running the real GBNF sampler, witnessed token `a` driven to -inf at decode step 0
> under the real (typed) grammar while the weakened (free-text) grammar permitted it.
> Full real generation confirms it: the typed grammar produced an in-type
> `Record { urgency: 0, action: Draft }` by construction; the weakened grammar wandered
> to free text (`"of 94.108, heav"`). A double-accept bug (llama_sampler_sample accepts
> internally) was found and fixed. Run:
> `WITCHCRAFT_GGUF=$PWD/models/<m>.gguf cargo test --features llama real_llama -- --nocapture`.
> Confidence (6.3) is now REAL: the geometric mean of the masked probability the sampler
> assigned each chosen token, read via a second grammar sampler advanced in lockstep (so the
> generation chain is never double-accepted). Live witness: 0.568 (in-type) vs 0.044
> (weakened), masking litmus still holds. §8 holds: this proves the type masks SHAPE, never
> quality — the confidence number quantifies the model's own certainty, not correctness.

## 7. Network engine — frontier API

- [x] 7.1 Add the frontier engine (Anthropic/OpenAI-style); credentials from env, never source/manifest-committed <!-- engine/frontier.rs, --features frontier -->
- [x] 7.2 Map our `Grammar` → JSON-Schema (enums for variants, min/max for refined ints, objects for records) <!-- engine::grammar_to_json_schema -->
- [x] 7.3 Default `grammar_constrained = true` but `litmus_safe = Some(false)` with reasons (no token-level mask); the falsification harness confirms via the no-trace path <!-- frontier describe; falsify -->
- [x] 7.4 The non-litmus-safe status is recorded and acted on by §A refusal — proven at the CLI (no live key needed: a strict need refuses *before* any network call) <!-- examples/strict_divine.witch + examples/manifests/triage.frontier.toml; witch run --features frontier refuses to start -->

> Group 7 status: frontier engine compiles behind `--features frontier` (ureq + serde_json).
> Strict-refusal **proven at runtime via the CLI**: `witch run examples/strict_divine.witch
> --manifest examples/manifests/triage.frontier.toml --features frontier` exits non-zero with
> `refuse to start: need 'cloud-triage-v1' is litmus-strict but engine 'frontier-large' is
> non-litmus-safe (...); add a source-visible downgrade to run anyway`. The validate-after
> engine cannot silently serve a strict need. A *live API call* is still not exercised offline;
> confidence is conservative pending logprobs.

## 8. Falsification test (HEADLINE — Verification B)

- [x] 8.1 Build a `divine` site twice (real vs weakened grammar) and capture the per-step permitted-token trace per engine <!-- engine::falsify; infer_traced -->
- [x] 8.2 Assert masking occurred: at ≥1 decode step a token the weakened grammar permits is forbidden by the real grammar; final-string comparison rejected <!-- falsify; tests/falsification.rs -->
- [x] 8.3 Fail loudly when masking cannot be demonstrated ("LITMUS FAILED"); runs against Mock (passes) and each real engine via the shared harness <!-- falsify; tests/falsification.rs -->
- [x] 8.4 An engine with no token-level visibility (frontier) yields no trace ⇒ marked non-litmus-safe with reasons (never silently safe) <!-- falsify no-trace path -->

## 9. Real confidence + provenance (Verification: model_version_or_sha)

- [x] 9.1 Surface engine-derived confidence into the existing discharge/fallback path (shape unchanged, source real) <!-- exec_divine uses result.confidence -->
- [x] 9.2 Populate provenance with `model`, `model_version_or_sha`, `backend_id`, seed, sampling, intent <!-- value::Provenance; runtime mirror -->
- [ ] 9.3 Test: changing the bound model artifact changes `model_version_or_sha` (covered structurally; engine-level test pending real engines)

## 10. Seed determinism honesty (Verification C)

- [x] 10.1 State in `--seed` CLI help (and README) that exact reproducibility holds only on the Mock engine; real engines are best-effort <!-- witch USAGE -->
- [x] 10.2 Provenance records seed + sampling so a real-engine run is explainable even when not bit-reproducible <!-- Provenance.sampling -->

> Group 9.3 / 11 require the real engines to actually run; see Group 6/7 status.

## 11. Cross-engine flagship (interpreter path)

- [x] 11.1 Deployment manifests for the flagship (`examples/manifests/triage.{laptop,cloud}.toml`), selecting the engine by manifest with zero source change <!-- offline: bind to Mock with distinct model ids; real llama/frontier documented inline -->
- [x] 11.2 Test: the same flagship source runs under two manifests, provenance reflects the bound model (engine swapped by manifest only); §8 honesty in README <!-- tests/manifest.rs flagship_swaps_engine_by_manifest -->

## 12. Validation

- [x] 12.1 `cargo fmt --all`, `cargo clippy --workspace` clean
- [x] 12.2 `cargo test --workspace` green (Mock-default suite fast/offline; real-engine paths feature-gated)
- [x] 12.3 `openspec validate add-inference-runtime --strict` clean
- [x] 12.4 README: the contract, manifest format, `permit(network)`, non-litmus-safe refusal + downgrade, falsification test, seed honesty, §8 caveat
