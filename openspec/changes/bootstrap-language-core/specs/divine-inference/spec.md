## ADDED Requirements

### Requirement: divine resolves a typed inference region
A `divine <name>: <OutputType> from (<inputs>) using <oracle>` block SHALL request that the oracle produce a value inhabiting `<OutputType>`, constrained during generation by that type (see constrained-decoder). The result SHALL be an `Inferred<OutputType>`. The `<inputs>` SHALL be evaluated in the enclosing scope before generation.

#### Scenario: divine produces an inferred value of the declared type
- **WHEN** `divine decision: Disposition from (msg) using triage ...` executes
- **THEN** `decision` is an `Inferred<Disposition>` whose underlying value structurally inhabits `Disposition`

#### Scenario: Inputs are evaluated before generation
- **WHEN** an input expression to `divine` is `"Explain ${topic}"` and `topic` is `tides`
- **THEN** the oracle receives the resolved input `Explain tides`

### Requirement: Confidence discharge and fallback
A `divine` block SHALL include a `with confidence >= <θ>` discharge and a `fallback <expr>`. When the inferred confidence meets the threshold, the block SHALL yield the value as a plain `<OutputType>`; otherwise it SHALL evaluate the `fallback` and the underlying value SHALL NOT flow downstream.

#### Scenario: Low confidence takes the fallback
- **WHEN** generation yields a value whose confidence is below the declared threshold
- **THEN** the `fallback` expression is evaluated and the low-confidence value does not reach subsequent statements

#### Scenario: Sufficient confidence yields the typed value
- **WHEN** generation yields a value whose confidence meets the threshold
- **THEN** the block yields a plain `<OutputType>` usable by following statements

### Requirement: Provenance threads into enact
The provenance carried by a discharged `divine` result SHALL remain attached through to `enact`, so that the executed action is associated with the oracle, inputs, and seed that produced it, without the program logging this by hand.

#### Scenario: Enacted action retains provenance
- **WHEN** a discharged `Disposition` is passed to `enact`
- **THEN** the enacted action is associated with the originating provenance record

### Requirement: The litmus property
Removing the declared output type from a `divine` block SHALL change the computation at the moment of inference (the generation is no longer constrained by the type). The type SHALL therefore be part of the computation, not a post-hoc validation applied after an otherwise-identical generation.

#### Scenario: Deleting the type changes generation
- **WHEN** the same `divine` program is run twice with the same decoder seed — once with output type `Disposition` and once with the type structurally removed/weakened
- **THEN** the generated output differs (the typed run is confined to `Disposition`; the untyped run is not), demonstrating the type constrains generation rather than validating it afterward
