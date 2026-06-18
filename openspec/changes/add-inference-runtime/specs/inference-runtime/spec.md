## ADDED Requirements

### Requirement: Inference is a pluggable engine the program is written against
The runtime SHALL define an `Engine` contract that all inference backends implement, and application programs SHALL be written against a NEED and a POLICY, never against a concrete engine or model. A NEED SHALL consist of a stable semantic intent id, the evaluated input, and the output-type grammar. A program SHALL NOT name an engine, vendor, or model; engine selection SHALL happen at load from a deployment manifest (see "Engine binding"). The language SHALL trust the contract, not any particular engine: every property the program relies on SHALL be one the contract requires of all engines.

#### Scenario: The same program runs on different engines unchanged
- **WHEN** a program is run once with a manifest binding its needs to a local engine, and again with a manifest binding the same needs to a network engine
- **THEN** the program source is byte-identical between the two runs and no recompilation of source is required to switch engines

#### Scenario: An engine is reached only through the contract
- **WHEN** any `divine` or embedding operation executes
- **THEN** it is served through the `Engine` contract (`describe`/`infer`/`embed`), and no caller depends on which concrete engine answered

### Requirement: The source names a semantic intent, never a model
The `oracle <name> = summon "<intent-id>"` string SHALL be a semantic intent name describing what the inference is FOR (e.g. `"TriageReasoner"`, `"EmotionReader"`), resolved to a concrete engine and model by the manifest. A model, vendor, or engine name appearing in application source SHALL be treated as a structural design violation of this contract. The intent id SHALL be what the manifest matches against and what provenance reports.

#### Scenario: Intent name resolves through the manifest
- **WHEN** source declares `oracle triage = summon "TriageReasoner"` and the manifest binds `TriageReasoner` to a concrete engine + model
- **THEN** the program uses that engine/model at the `divine` site while the source continues to name only `TriageReasoner`

#### Scenario: Intent name is legible to a human auditor
- **WHEN** a reader audits a program's inference sites
- **THEN** each site names what the inference is for (the intent), not an opaque handle or a model id, so the reader can see the purpose without consulting deployment config

> Note: this is a deliberate sharpening of the paper's literal §5.1 example `summon "llama3"` (a model name) in service of §9.1 swappability — so that applications outlive the models they once used. It is intentional, not an oversight.

### Requirement: Every legal engine guarantees grammar-by-construction
The `Engine` contract SHALL require that `infer` return a value inhabiting the supplied output grammar **by construction** — the grammar SHALL constrain generation token-by-token (e.g. logit-masking / GBNF) so illegal outputs are unreachable. An engine SHALL declare via `describe()` whether it supports grammar-constrained decoding. An engine that cannot enforce the grammar during generation SHALL NOT be a legal engine for a grammar-constrained need and SHALL be rejected at registration for such needs. Validate-after-generation followed by resampling SHALL NOT satisfy this requirement.

#### Scenario: A grammar-incapable engine is rejected
- **WHEN** an engine reports `grammar_constrained = false` and is bound to a `divine` need
- **THEN** registration fails with a diagnostic naming the engine and the need, and the program does not run on that binding

#### Scenario: Generated value inhabits the type by construction
- **WHEN** a litmus-safe engine serves a `divine` of a refined or variant output type
- **THEN** the returned value inhabits the type because out-of-grammar tokens were never generable, not because an out-of-grammar value was generated and then discarded

### Requirement: Non-litmus-safe engines are first-class and a strict need refuses them
An engine that constrains output by server-side validation or schema enforcement rather than true token-level masking (i.e. one that cannot demonstrate grammar-by-construction) SHALL be marked **non-litmus-safe** in its `describe()`, with the reason recorded. A `divine` need SHALL be **litmus-strict by default**: binding a non-litmus-safe engine to a litmus-strict need SHALL cause the program to **refuse to start** at load, with a diagnostic naming the need, the engine, and the reason — exactly as a locality policy no-match refuses. Running a need on a non-litmus-safe engine SHALL require an explicit, source-visible acknowledgement on that need (a downgrade the author opts into and a reader can see); absent that acknowledgement, validate-after SHALL NOT be reachable.

#### Scenario: Strict need refuses a non-litmus-safe engine
- **WHEN** a litmus-strict `divine` need is bound by manifest to an engine marked non-litmus-safe
- **THEN** the program refuses to start with a diagnostic naming the need, the engine, and why it is non-litmus-safe

#### Scenario: Downgrade is explicit and legible
- **WHEN** an author intends to run a need on a non-litmus-safe engine
- **THEN** the source must carry an explicit acknowledgement on that need, so the relaxation of the by-construction guarantee is visible to any reader and is never silent

### Requirement: Confidence and provenance are produced by the engine
`infer` SHALL return a confidence scalar derived from the engine's own generation signal (e.g. token logprobs over the constrained decode), never synthesised by the caller, and a provenance record containing at least `{ model_id, model_version_or_sha, backend_id, seed, sampling }`. `model_version_or_sha` SHALL identify the exact model artifact (a content hash for a local model, or the provider's model version for a network model) so that a model version change is detectable from provenance alone.

#### Scenario: Provenance flags a model version change
- **WHEN** the same program is run against two different versions of the model bound to a need
- **THEN** the two runs' provenance records differ in `model_version_or_sha`, making the change detectable without source instrumentation

#### Scenario: Confidence comes from the engine
- **WHEN** an inferred value is produced
- **THEN** its confidence reflects the engine's generation signal and is not a constant or caller-fabricated value

### Requirement: Engine binding and load-time resolution from a manifest
The deployment SHALL provide a manifest, separate from program source, that binds each intent id to a concrete engine and model and records each engine's kind and parameters. Models SHALL be named only in the manifest. At program load, each need used by the program SHALL be resolved against the manifest and checked against the program's POLICY (locality, latency, litmus-strictness). Resolution SHALL happen once at load, not per call.

#### Scenario: Same program, different manifest, different deployment
- **WHEN** one manifest binds needs to a local engine and another binds them to a network engine
- **THEN** the same program source runs in both deployments, differing only by manifest

#### Scenario: No matching engine refuses to start
- **WHEN** a need cannot be satisfied by any engine compatible with the program's policy (e.g. on-device-only on a machine with no local engine)
- **THEN** the program refuses to start with a diagnostic naming the need and the unmet constraint, and never silently substitutes an engine that violates the policy

### Requirement: Network access is a source-visible locality policy
On-device-only SHALL be the default policy for a need. Network-capable engines SHALL be eligible for a need only if the need's site is granted `permit(network)` (reusing capability-effects). Absence of `permit(network)` SHALL mean a network engine MUST NOT be selected for that need, even if one is bound in the manifest; the mismatch SHALL refuse to start rather than silently use the network.

#### Scenario: Network engine requires permit(network)
- **WHEN** a manifest binds a need to a network engine but the need's site does not grant `permit(network)`
- **THEN** the program refuses to start, reporting that the binding would require network access the source did not permit

#### Scenario: Permitted network use is legible
- **WHEN** a need is allowed to use the network
- **THEN** `permit(network)` is visible at the source site, so a reader can see which inferences may leave the device

### Requirement: Tier is advisory and never blocks
An engine MAY declare one or more capability tiers in `describe()`. A POLICY MAY state an advisory minimum tier. Tier SHALL be used only to prefer or rank engines during selection and SHALL NEVER cause a refusal to start or otherwise gate execution. This change SHALL NOT define a precise tier metric; the tier value SHALL be an engine-declared advisory string only.

#### Scenario: Tier preference does not block
- **WHEN** no bound engine meets an advisory minimum tier but one satisfies all hard constraints (locality, litmus-strictness)
- **THEN** the program starts using that engine; the unmet tier preference does not cause a refusal

### Requirement: The deterministic Mock is retained as a selectable test engine
The deterministic, grammar-respecting decoder SHALL be retained as a selectable engine (`kind = "mock"`) implementing the same `Engine` contract, and SHALL remain the default when no manifest or model is present. It SHALL serve as the deterministic litmus oracle in tests and as the offline default for first run, examples, and CI. It SHALL NOT be removed.

#### Scenario: Offline default is the Mock engine
- **WHEN** a program runs with no manifest and no model available
- **THEN** it runs deterministically on the Mock engine, offline, producing the same output every run for a fixed seed

### Requirement: A real local engine runs inference under the contract
The runtime SHALL ship a real local engine that performs inference with a downloaded model and enforces the output grammar token-by-token during generation, served entirely through the `Engine` contract. The language and toolchain SHALL remain self-contained: only model execution SHALL link an external library, and only when such an engine is selected. The model artifact SHALL sit beside the binary and SHALL NOT be bundled into the toolchain.

#### Scenario: Local inference enforces the grammar during generation
- **WHEN** the local engine serves a `divine` of a constrained output type with a real model
- **THEN** the generated value inhabits the type by construction, and the falsification test (below) passes for this engine

### Requirement: A network engine runs inference under the contract
The runtime SHALL ship a network engine (a frontier API backend) implementing the same `Engine` contract, mapping the output grammar to the provider's structured-output mechanism. Whether this engine is litmus-safe SHALL be determined empirically by the falsification test, not asserted by fiat: if the test passes it SHALL be marked litmus-safe; otherwise it SHALL be marked non-litmus-safe with the reason recorded. Credentials SHALL come from the environment and SHALL NOT appear in source or be committed in the manifest.

#### Scenario: Network engine litmus status is test-determined
- **WHEN** the falsification test is run against the network engine
- **THEN** the engine is marked litmus-safe if and only if masking can be demonstrated; if it cannot, the engine is marked non-litmus-safe with reasons, not silently treated as safe

### Requirement: The falsification test asserts masking occurred during generation
The change SHALL provide a falsification test that, for each engine claiming grammar-constrained decoding, builds a `divine` site twice — once with the real output-type grammar and once with the weakened grammar — and SHALL assert, by inspecting the decode trace, that at one or more decode steps a token the weakened grammar would permit was actively forbidden by the real grammar. The assertion SHALL be that masking occurred during generation; comparing only final output strings SHALL be insufficient and SHALL NOT satisfy the test. If masking cannot be demonstrated, the test SHALL fail loudly and report that the engine is a wrapper, not AI-first.

#### Scenario: Masking is demonstrated at the token level
- **WHEN** the falsification test runs against a litmus-safe engine
- **THEN** it identifies at least one decode step where the real grammar forbade a token the weakened grammar permitted, proving the type constrained generation rather than validating it afterward

#### Scenario: Indistinguishable masking fails loudly
- **WHEN** real and weakened grammars produce no observable difference in permitted tokens at any decode step for an engine
- **THEN** the test fails with a message stating the litmus has failed for that engine and the engine behaves as a wrapper

#### Scenario: The test runs per engine, not only the Mock
- **WHEN** the workspace test suite runs
- **THEN** the falsification test is executed against each real engine that claims grammar-constrained decoding, in addition to the Mock oracle

### Requirement: Seed determinism honesty is user-visible
User-facing documentation and CLI help SHALL state plainly that `--seed` is a determinism contract only with the Mock test engine, and that with real local or network engines a seed is best-effort and SHALL NOT be assumed to reproduce output exactly across machines, quantisations, or providers. Provenance SHALL record the seed and sampling so a run is explainable even when not bit-reproducible.

#### Scenario: Docs state the seed contract boundary
- **WHEN** a user reads the `--seed` documentation or CLI help
- **THEN** it states that exact reproducibility holds for the Mock engine and is best-effort for real engines, so nondeterminism on a real engine is not mistaken for a defect
