## ADDED Requirements

### Requirement: Oracle as a first-class typed value
`oracle <name> = summon "<model-id>"` SHALL bind a first-class `oracle` value carrying a model identifier and a reference to the active decoder. An `oracle` value SHALL be bindable with `let`/`var`, passable as a function argument, and returnable from a function, and SHALL have an `oracle` type distinct from any ordinary handle/client type.

#### Scenario: Summon an oracle
- **WHEN** a program executes `oracle triage = summon "support-reasoner-v3"`
- **THEN** `triage` is a value of type `oracle` carrying the model id `support-reasoner-v3`

#### Scenario: Oracle passed to a function
- **WHEN** an `oracle` value is passed as an argument to a `fn`
- **THEN** the function receives it typed as `oracle` and may use it in a `divine` block

### Requirement: Inference is a typed effect producing an inferred value
Inference performed through an oracle (within a `divine` block) SHALL be typed as an effect, distinct from pure computation, and SHALL produce an `Inferred<T>` for the requested output type `T` rather than a bare `T` or a raw string. The oracle SHALL NOT expose a string-returning call that bypasses the inferred-value type.

#### Scenario: Inference yields an inferred value
- **WHEN** a `divine` block requests output type `Disposition` using an oracle
- **THEN** the produced value has type `Inferred<Disposition>`, not `Disposition` and not `glyph`

#### Scenario: No untyped string escape hatch
- **WHEN** a program attempts to obtain a raw string directly from an oracle outside the inferred-value/`divine` machinery
- **THEN** no such operation is available in the v0.1 surface (inference is only reachable as a typed, inferred-value-producing effect)

### Requirement: Provenance origin
An oracle SHALL contribute its model identifier (and the decoder seed in use) to the provenance record of any inferred value it produces.

#### Scenario: Provenance names the oracle
- **WHEN** an inferred value is produced via oracle `triage`
- **THEN** its provenance record identifies `triage`'s model id and the seed used to generate it
