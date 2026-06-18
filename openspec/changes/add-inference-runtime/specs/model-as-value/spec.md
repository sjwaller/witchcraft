## MODIFIED Requirements

### Requirement: Oracle as a first-class typed value
`oracle <name> = summon "<intent-id>"` SHALL bind a first-class `oracle` value carrying a **semantic intent id** (what the inference is for, e.g. `"TriageReasoner"`) and a reference to the engine bound to that intent by the manifest. The string SHALL NOT be a model, vendor, or engine name; a model name in application source SHALL be a structural design violation (see inference-runtime). An `oracle` value SHALL be bindable with `let`/`var`, passable as a function argument, and returnable from a function, and SHALL have an `oracle` type distinct from any ordinary handle/client type.

#### Scenario: Summon an oracle by intent
- **WHEN** a program executes `oracle triage = summon "TriageReasoner"`
- **THEN** `triage` is a value of type `oracle` naming the intent `TriageReasoner`, which the manifest resolves to a concrete engine and model

#### Scenario: Oracle passed to a function
- **WHEN** an `oracle` value is passed as an argument to a `fn`
- **THEN** the function receives it typed as `oracle` and may use it in a `divine` block

### Requirement: Provenance origin
An oracle SHALL contribute to the provenance record of any inferred value it produces at least the resolved `model_id`, the `model_version_or_sha` of the exact model artifact, the `backend_id` of the engine that served the inference, and the seed and sampling in use. The provenance SHALL identify the intent the oracle names, so a reader can connect the inferred value to its purpose, its engine, and its exact model version.

#### Scenario: Provenance names the engine and model version
- **WHEN** an inferred value is produced via oracle `triage`
- **THEN** its provenance record identifies the intent `TriageReasoner`, the resolved `model_id` and `model_version_or_sha`, the `backend_id`, and the seed used

#### Scenario: Provenance distinguishes model versions
- **WHEN** the bound model artifact changes between two runs
- **THEN** the provenance `model_version_or_sha` differs between the runs, making the change detectable from provenance alone
