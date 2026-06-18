## MODIFIED Requirements

### Requirement: divine resolves a typed inference region
A `divine <name>: <OutputType> from (<inputs>) using <oracle>` block SHALL request that the engine bound to the oracle's intent produce a value inhabiting `<OutputType>`, constrained **during generation** by that type (see constrained-decoder). The `<inputs>` SHALL be evaluated in the enclosing scope before generation and SHALL be passed to the engine as the inference input. The result SHALL be an `Inferred<OutputType>` carrying confidence and provenance from the engine. A `divine` site SHALL be litmus-strict by default: it SHALL only run on an engine that guarantees the grammar by construction, unless the source carries an explicit, visible downgrade acknowledgement (see inference-runtime).

#### Scenario: divine produces an inferred value of the declared type
- **WHEN** `divine decision: Disposition from (msg) using triage ...` executes on a litmus-safe engine
- **THEN** `decision` is an `Inferred<Disposition>` whose underlying value inhabits `Disposition` by construction

#### Scenario: Inputs are passed to the engine before generation
- **WHEN** an input expression to `divine` is `"Explain ${topic}"` and `topic` is `tides`
- **THEN** the resolved input `Explain tides` is passed to the bound engine as the inference input rather than discarded

#### Scenario: Strict site refuses a non-litmus-safe engine
- **WHEN** a litmus-strict `divine` site is bound to a non-litmus-safe engine and carries no downgrade acknowledgement
- **THEN** the program refuses to start, reporting the site, the engine, and why it is non-litmus-safe

### Requirement: The litmus property
Removing or weakening the declared output type from a `divine` block SHALL change the computation at the moment of inference on every legal engine — the generation is no longer constrained by the type. The type SHALL be part of the computation, not a post-hoc validation applied after an otherwise-identical generation, and this property SHALL hold against real engines, not only the deterministic Mock. The falsification test (see inference-runtime) SHALL demonstrate, per engine, that the real grammar forbade tokens the weakened grammar permitted during generation.

#### Scenario: Deleting the type changes generation on a real engine
- **WHEN** the same `divine` program is run against a real litmus-safe engine with the same seed — once with output type `Disposition` and once with the type structurally weakened
- **THEN** generation differs, and the falsification test identifies at least one decode step where the typed run forbade a token the weakened run permitted

#### Scenario: Failure to constrain is reported, not hidden
- **WHEN** an engine cannot demonstrate that the type constrained generation (real and weakened are indistinguishable at the token level)
- **THEN** the falsification test fails loudly for that engine and the engine is treated as non-litmus-safe rather than silently accepted
