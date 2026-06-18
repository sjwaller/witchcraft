## MODIFIED Requirements

### Requirement: Decoder interface
The runtime SHALL define an `Engine` contract (superseding the v0.1 `Decoder` interface) that produces a value by generating tokens constrained by a supplied grammar. `infer` SHALL receive the inference intent id, the evaluated input, the output-type grammar, and the active policy, and SHALL return the generated value together with a confidence scalar and a provenance record. The input SHALL be threaded to the engine (the v0.1 interface omitted it); inference SHALL act on the supplied input, not ignore it. Multiple engines (a deterministic Mock, a real local engine, and a network engine) SHALL implement the same contract, and `divine`/embedding call sites SHALL use whichever engine is bound without source changes.

#### Scenario: Engine returns value, confidence, and provenance
- **WHEN** an engine is asked to generate against a compiled grammar for a given intent id and input
- **THEN** it returns a value admitted by that grammar together with a confidence scalar and a provenance record

#### Scenario: Engine is swappable behind the contract
- **WHEN** a different engine implementation is bound for a need
- **THEN** existing `divine`/oracle call sites use it without source changes

#### Scenario: Input reaches the engine
- **WHEN** a `divine` site supplies input to inference
- **THEN** the bound engine receives that input as part of the `infer` request rather than generating independently of it

### Requirement: Deterministic, grammar-respecting reference decoder
The deterministic, grammar-respecting decoder SHALL be retained as the `Mock` engine implementing the `Engine` contract: deterministic given a fixed seed, honouring the supplied grammar token-by-token so it can only emit values admitted by the grammar, and making no network calls. It SHALL remain the offline default when no manifest or model is present and the deterministic oracle against which the falsification test is checked. Real engines SHALL enforce the grammar by construction during generation (not by validate-after-resample); an engine that cannot SHALL be marked non-litmus-safe (see inference-runtime).

#### Scenario: Determinism under a fixed seed
- **WHEN** the Mock engine generates against the same grammar with the same seed twice
- **THEN** it produces identical output both times

#### Scenario: Illegal outputs are unreachable
- **WHEN** the Mock engine generates against a grammar compiled from `one_of { Draft, Escalate }`
- **THEN** it can only emit `Draft` or `Escalate`; a value outside the type is never produced

#### Scenario: No network access for the Mock engine
- **WHEN** any decode occurs on the Mock engine
- **THEN** it is resolved locally without a network request
