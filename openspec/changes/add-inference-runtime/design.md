# Design — add-inference-runtime

> This is the change that decides whether Witchcraft is a real AI-first language or an
> LLM wrapper. The discriminator (§4) is the litmus (§6.3): **delete the type and
> generation changes**. Today that holds only against a mock built to respect the
> grammar. Here we make it hold against *real* decoders — or prove it cannot, loudly.

## Context

Current state (read from the tree):

- `crates/witchcraft-runtime/src/decode.rs` — a deterministic, grammar-respecting PRNG.
  It is the *only* decoder. It respects the grammar by construction because it was
  written to walk the grammar; nothing about a real tokenizer is exercised.
- `crates/witchcraft-runtime/src/abi.rs` — `w_divine(grammar, oracle, model, conf_out)`.
  **There is no input/prompt parameter.** The `from (...)` inputs never reach the decoder.
- `crates/witchcraft/src/decoder.rs` — the interpreter's `Decoder` trait; also no input.
- `crates/witchcraft-codegen/src/lib.rs` — codegen for `divine` evaluates the input
  expressions "for effect" and discards them; confidence is synthesised.

So three things are simultaneously true: (a) the litmus is unproven on a real model,
(b) the prompt is structurally dropped, (c) confidence/provenance are fabricated. This
change fixes all three behind one contract.

What we keep (archived, do not relitigate): the `divine`-only surface (no
`oracle.invoke` returning a free string — §5.2's "every inference is typed"); the
grammar compiler from output types; the weakened-grammar lowering used by the litmus;
the discharge/confidence/fallback rules; the offline-mock-default distribution contract.

## Goals / Non-Goals

**Goals**
1. A falsification test that would *disprove* AI-first, run per real engine (headline).
2. A four-layer pluggable contract: NEED, POLICY, BINDING, ENGINE — app written against
   a need, never a model; smarter engine plugged in with no source change.
3. Two real engines (llama.cpp local, frontier network) + the demoted mock test engine.
4. Real confidence + provenance (`model_id, model_version_or_sha, backend_id, seed,
   sampling`); load-time engine resolution; refuse-to-start on policy no-match.
5. `permit(network)` as the source-visible locality policy (reuse capability-effects).

**Non-Goals**
- Making any model's output *correct* (§8 — out of scope by thesis).
- A precise tier metric (advisory string only; deferred, blocks nothing).
- Native lowering of memory/embedding/familiar (Break 2 → `complete-native-compile`),
  though `Engine::embed` is defined here so that change can consume it.
- Splitting `oracle` into per-modality types (§5.5) — kept as one type for v0.2; the
  contract is shaped not to foreclose it.

## The four layers (only the first two appear in source)

```
  APPLICATION SOURCE                         DEPLOYMENT                IMPLEMENTER
  ┌────────────────────────┐   ┌──────────────────────────┐   ┌────────────────────┐
  │ 1. NEED                 │   │ 3. BINDING (manifest)    │   │ 4. ENGINE TRAIT    │
  │   oracle triage =       │   │   [need.triage]          │   │  describe()        │
  │     summon "triage"     │──▶│   engine = "llama-local" │──▶│   tiers/modalities │
  │   divine d: Disposition │   │   model  = "qwen2.5-..." │   │   LOCAL|NETWORK    │
  │     from (msg) using    │   │   (model names live      │   │   latency_class    │
  │        triage           │   │    ONLY here)            │   │   grammar=MANDATORY │
  │                         │   │                          │   │  infer(need,policy)│
  │ 2. POLICY               │   │  resolve at LOAD:        │   │   -> value inhabits │
  │   permit(network)? no   │   │   need+policy → engine   │   │      grammar BY     │
  │   latency interactive   │   │   no match ⇒ REFUSE START│   │      CONSTRUCTION   │
  │   tier >= standard (adv)│   │                          │   │  + confidence/prov  │
  └────────────────────────┘   └──────────────────────────┘   └────────────────────┘
        legible to author            swap = new laptop/edge/        Mock|Llama|Frontier
        no model names ever          GPU/cloud, no source change     all satisfy trait
```

The boundary is the whole point: **NEED + POLICY are what the program promises; BINDING
is what this deployment provides; the ENGINE is interchangeable.** An app outlives the
models it once used because the only place a model is named is the manifest.

---

## Decision D1 — Source names a NEED, never a model

The `oracle`'s summon string becomes a **logical need id**, resolved by the manifest:

```
oracle triage = summon "triage"          // "triage" is a NEED id, not "gpt-x" or "qwen-y"
```

- The `intent_id` of an inference is the need handle (`triage`); the NEED is
  `(intent_id, input, output_grammar)`. This reuses the existing surface — no new
  syntax for the need itself.
- A literal vendor/model name in source is a **design violation**. v0.2 treats the
  string as opaque (no heuristic "is this a model name?" detection — that is brittle);
  the manifest is structurally the *only* place models are named, which is what makes
  the violation hard to commit. (Open Q4 — lean: opaque need-id, revisit a lint later.)
- This reinterprets the paper's literal §5.1 (`summon "llama3"`, a model name) in
  service of the paper's *deeper* thesis (§9.1 swappability / app outlives model). Call
  this out explicitly in the spec so the divergence from the example is intentional.

## Decision D2 — POLICY is source-visible constraints, not engine choices

POLICY lives in source because the author owns the *constraints* (§9.1). Minimal surface:

- **Locality** — the hard constraint. `permit(network)` (capability-effects) means
  network engines are *allowed*; its absence means **on-device-only**. This is the
  constraint that can cause refuse-to-start.
- **Latency class** + **min tier** — optional, declared on the `summon`:

```
oracle triage = summon "triage" with policy { latency interactive, tier >= standard }
```

  `latency` is a hard hint used in matching/ranking; `tier` is **advisory only** —
  used to *prefer* an engine, never to gate or refuse. (Resolves the apparent
  contradiction: refuse-to-start applies to locality/latency hard constraints; tier
  never blocks — Open Q5.)

Decision: keep the policy clause tiny and optional. If a program states no policy,
default is on-device-only + no latency/tier constraint. Network access is *opt-in* and
*legible*, satisfying §9.1.

## Decision D3 — The ENGINE trait (the contract everything trusts)

```rust
pub struct EngineDescription {
    pub backend_id: String,
    pub tiers: Vec<String>,            // advisory, e.g. ["standard"]
    pub modalities: Vec<Modality>,     // v0.2: [Text]
    pub locality: Locality,            // Local | Network
    pub latency_class: LatencyClass,   // Interactive | Batch | ...
    pub grammar_constrained: bool,     // MANDATORY. false ⇒ NOT a legal engine
}

pub struct InferRequest<'a> {
    pub intent_id: &'a str,
    pub input: &'a Value,              // the prompt the source dropped today
    pub output_grammar: &'a Grammar,   // compiled from the output type
    pub policy: &'a Policy,
    pub seed: Option<u64>,
}

pub struct InferResult {
    pub value: Value,                  // INHABITS output_grammar BY CONSTRUCTION
    pub confidence: f64,               // from the engine (logprobs), never synthesised
    pub provenance: Provenance,        // {model_id, model_version_or_sha, backend_id, seed, sampling}
}

pub trait Engine {
    fn describe(&self) -> EngineDescription;
    fn infer(&self, req: InferRequest) -> Result<InferResult, EngineError>;
    fn embed(&self, intent_id: &str, input: &Value, space: &str)   // for complete-native-compile
        -> Result<Embedding, EngineError>;
}
```

- **Universal property:** `infer` must return a value that inhabits `output_grammar` by
  construction — enforced *during* generation (logit-mask / GBNF), never
  validate-after-resample. An engine with `grammar_constrained = false` is rejected at
  registration; it is not a legal Witchcraft engine.
- The interpreter `Decoder` trait and the runtime ABI both collapse onto this contract.
- `embed` is declared now (so the sibling change consumes it) but only the mock + the
  existing deterministic hash implement it in this change; real embedding execution is
  the local engine's job once wired.

## Decision D4 — The ABI evolves to carry the prompt

`w_divine(grammar, oracle, model, conf_out)` →

```c
// stable, versioned C ABI used by both interpreter and compiled binaries
WitchStatus witch_ai_infer(
    const WitchInferRequest* req,   // intent_id, input value, grammar, policy, seed
    WitchInferResult*        out);  // value, confidence, provenance
```

Threading `input` through the ABI is what closes Break 3 at the boundary. The compiled
codegen stops dropping the `from (...)` inputs (the lowering work itself lands fully in
`complete-native-compile`; here the ABI shape + the interpreter path are delivered).
Version the struct (leading `abi_version` field) so engines and binaries can detect skew.

## Decision D5 — BINDING: the manifest + load-time resolution

A small TOML manifest, beside the binary, is the *only* place models are named:

```toml
[need.triage]
engine  = "llama-local"          # which engine
model   = "qwen2.5-3b-instruct"  # model id (manifest-only)
sha256  = "…"                    # → provenance.model_version_or_sha
locality = "local"
latency  = "interactive"
tier     = "standard"            # advisory

[engine.llama-local]
kind = "llama-cpp"
gguf = "./models/qwen2.5-3b-instruct.Q4_K_M.gguf"

[engine.frontier]
kind = "anthropic"               # or "openai"; api key from env, never source
```

**Resolution at LOAD (not per-call):**
1. For each need used by the program, find its binding.
2. Check the bound engine's `describe()` against the program's POLICY: locality must
   satisfy `permit(network)`/on-device-only; latency must be compatible; tier is used to
   *prefer* but never reject.
3. The bound engine must report `grammar_constrained = true`.
4. **No match ⇒ the program refuses to start** with a precise diagnostic naming the need,
   the unmet constraint, and the available engines. Never silently fall back across a
   policy boundary (e.g. never reach for the network when on-device-only).

"Same program, different manifest" = laptop / edge / GPU / cloud, with no source change.
This is the swappability proof.

## Decision D6 — Confidence + provenance are real

- **Confidence** is derived from the engine's token logprobs over the constrained
  decode (e.g. mean/length-normalised sequence probability), surfaced as the `f64` the
  discharge/fallback rules already consume. The *shape* of confidence is unchanged; only
  its *source* becomes real.
- **Provenance** records `{ model_id, model_version_or_sha, backend_id, seed, sampling }`.
  `model_version_or_sha` is the manifest `sha256` (local) or the provider's model
  version string (network). This is exactly what §6.2 needs to *flag a model version
  change via provenance* — a fabricated provenance could never satisfy that bar.

## Decision D7 — Engines to ship

| Engine    | Locality | Grammar mechanism                         | Litmus-safe? | Role |
|-----------|----------|-------------------------------------------|--------------|------|
| `Mock`    | Local    | walks the `Grammar` directly (PRNG)       | yes (oracle) | test/offline default |
| `Llama`   | Local    | GBNF compiled from `Grammar` (llama.cpp)  | yes (target) | first real engine |
| `Frontier`| Network  | JSON-Schema / structured-output from `Grammar` | **decided by the test** | swappability proof |

- **`Llama` (lean: `llama-cpp-2` FFI, GGUF, GBNF).** Links `libllama` (C/C++) in CI.
  Acceptable because the *language/toolchain* stays self-contained — only model
  *execution* links a lib, and only when a real local model is selected; the default
  offline path is still the pure-Rust `Mock`. GGUF artifact sits beside the binary,
  fetched out-of-band (never bundled — honours the distribution change).
  - Alternative considered: pure-Rust `candle` (no FFI, but we'd author logit-masking by
    hand and own the litmus-critical code path). Rejected for v0.2: GBNF is the *safest*
    way to prove Break 1; we want the litmus to pass on battle-tested constrained decode
    before trusting our own masker. Revisit `candle` once the litmus is green.
- **`Frontier` (Anthropic/OpenAI-style).** Map `Grammar` → JSON-Schema (enums for
  variants, `minimum`/`maximum` for refined int ranges, objects for records). Providers
  enforce structured output *server-side*; whether that is grammar-*by-construction* or
  validate-after is **not asserted by fiat** — the falsification test decides empirically
  per provider. If it passes, the engine is litmus-safe; if it only validates-after, it
  is marked `non-litmus-safe` (still usable, but excluded from the litmus guarantee) with
  the reason recorded. API keys come from env, never source/manifest-committed.
- **`Mock` demoted.** Same trait, selectable as `kind = "mock"`. Remains the default
  when no manifest/model is present, keeping all existing tests + offline first-run
  deterministic. Never deleted — it is the deterministic *oracle* the litmus compares
  against.

## Decision D8 — The falsification test (headline, fully specified)

This is the §6.2 fault-injection bar for this change and the permanent canary.

```
For each engine E in {Llama, Frontier} that claims grammar_constrained:
  pick a divine site S with a constrained output type T (e.g. Disposition, or
      `spark in 0..10`)
  G_real     = compile_grammar(T)
  G_weak     = weaken(G_real)          // existing weakened-grammar lowering
  fix seed, input, sampling

  R_real = trace_decode(E, S.input, G_real)   // capture per-token candidate masking
  R_weak = trace_decode(E, S.input, G_weak)

  ASSERT in-grammar-by-construction:
     every emitted token of R_real was permitted by G_real at that step
     AND R_real.value inhabits T                      // type membership, not regex-after
  ASSERT the type PARTICIPATES (litmus):
     R_real and R_weak are DISTINGUISHABLE —
       under G_weak, at least one step makes an out-of-grammar token reachable
       (token-mask differs) OR the produced value would not inhabit T.
  IF R_real and R_weak are indistinguishable:
     FAIL LOUDLY: "litmus failed for engine E — the type did not participate in
       generation; this engine is a wrapper, not AI-first."
```

Key properties:
- It inspects the **decode trace** (which tokens the grammar permitted at each step),
  not just the final string — so it proves *enforcement during generation*, not a
  post-hoc regex pass.
- It runs against **each real engine**. The `Mock` runs it too (it must pass trivially —
  if the mock ever fails, the *test harness* is wrong, a useful guard).
- For `Frontier`, if the provider gives no token-level visibility, the test degrades to
  the strongest available evidence (does weakening the schema change what the model can
  return?). If even that is indistinguishable, `Frontier` is recorded
  **non-litmus-safe** — an honest, documented outcome, not a hidden failure.

## Decision D9 — Determinism honesty (§8)

- `--seed` is **fully** deterministic only on `Mock`.
- Real engines are reproducible **within** the same engine + model build + device +
  sampling, and **not** across machines/quantisations. The spec states this plainly;
  provenance carries `seed` + `sampling` so a run is *explainable* even when not
  bit-identical elsewhere. We never claim more than is true.

## Risks / Trade-offs

- **llama.cpp C++ in CI.** Accepted and named: the toolchain stays Rust; only model
  execution links a lib, and only when selected. CI caches the build; default tests use
  `Mock` so the suite stays fast and FFI-free unless explicitly exercising `Llama`.
- **Frontier may be non-litmus-safe.** Treated as a *finding*, not a blocker — the whole
  point of the test is to tell the truth about an engine. The contract still holds; that
  engine is simply outside the litmus guarantee, recorded with reasons.
- **GBNF / JSON-Schema can't express every grammar feature.** If a refined type can't be
  mapped losslessly to the engine's constraint format, that engine **cannot serve that
  need** (refuse, don't downgrade to validate-after). Mapping coverage is part of the
  engine's `describe()` surface area; gaps are explicit, never silent.
- **ABI churn.** `witch_ai_infer` is versioned from day one to absorb future fields
  (modalities, streaming) without breaking shipped binaries.

## Migration / Compatibility

- Default behaviour is unchanged: no manifest ⇒ `Mock` ⇒ every existing example stays
  deterministic + offline (honours `add-distribution`).
- Real engines are strictly opt-in via the manifest. Existing `oracle ... = summon "…"`
  programs keep working; the string is simply *reinterpreted* as a need-id (D1) and the
  binding/model live in the manifest.

## Dependency graph

```
bootstrap-language-core ─┐
add-grimoire-codegen ────┤
add-capability-effects ──┼─▶ add-inference-runtime ─▶ complete-native-compile
add-distribution ────────┘        (this change)            (Break 2)
```

`add-inference-runtime` is sequenced **before** `complete-native-compile`: the engine
`infer`/`embed` ABI and the contract must exist before the AI primitives can lower to
native code and the compiled litmus can be proven.

## What "finished" means for THIS change (Break 1 + 3)

- The contract (NEED/POLICY/BINDING/ENGINE) exists; resolution at load; refuse-to-start
  on policy no-match.
- `Llama` runs real inference under `witch run`; the **falsification test passes** for
  `Llama`; `Frontier` either passes or is honestly marked non-litmus-safe with reasons.
- Real confidence + provenance (incl. `model_version_or_sha`).
- The flagship runs end-to-end against a real **local** model and the **frontier** API,
  selected purely by manifest, with **no source change** between runs (interpreter path).
- §8 honesty loud throughout. `cargo fmt`/`clippy`/`test --workspace` +
  `openspec validate --strict` clean.

(The *compiled* `compiled == interpreted` proof of the same, across all four primitives,
is owned by `complete-native-compile`, which depends on this change.)
