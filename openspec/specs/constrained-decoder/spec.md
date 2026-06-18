# constrained-decoder Specification

## Purpose
TBD - created by archiving change bootstrap-language-core. Update Purpose after archive.
## Requirements
### Requirement: Output types compile to a generation grammar
The runtime SHALL compile an inference output type into a generation grammar: a record type into an ordered field grammar; a sum type `one_of { ... }` into an alternation over exactly its declared variants; a refinement `spark in a..b` into a numeric production bounded to that range; a `glyph` into a bounded text production. The compiled grammar SHALL admit exactly the values that inhabit the type.

#### Scenario: Variant type compiles to a closed alternation
- **WHEN** the type `one_of { Draft, Escalate, AskClarify }` is compiled to a grammar
- **THEN** the grammar admits exactly those three variants and no others

#### Scenario: Refinement compiles to a bounded range
- **WHEN** the type `spark in 0..10` is compiled to a grammar
- **THEN** the grammar admits only numeric values within 0..10

### Requirement: Decoder interface
The runtime SHALL define a `Decoder` interface that produces a value by generating tokens constrained by a supplied grammar, returning the generated value together with a confidence scalar. v0.1 SHALL provide a reference implementation; real model backends SHALL be addable later by implementing the same interface without changes to callers.

#### Scenario: Decoder returns value and confidence
- **WHEN** the decoder is asked to generate against a compiled grammar
- **THEN** it returns a value admitted by that grammar together with a confidence scalar

#### Scenario: Backend is swappable behind the interface
- **WHEN** a new decoder implementation is provided
- **THEN** existing `divine`/oracle call sites use it without source changes

### Requirement: Deterministic, grammar-respecting reference decoder
The v0.1 reference decoder SHALL be deterministic given a fixed seed and SHALL honour the supplied grammar token-by-token, so that it can only emit values admitted by the grammar. It SHALL NOT make network calls.

#### Scenario: Determinism under a fixed seed
- **WHEN** the reference decoder generates against the same grammar with the same seed twice
- **THEN** it produces identical output both times

#### Scenario: Illegal outputs are unreachable
- **WHEN** the reference decoder generates against a grammar compiled from `one_of { Draft, Escalate }`
- **THEN** it can only emit `Draft` or `Escalate`; a value outside the type is never produced

#### Scenario: No network access
- **WHEN** any decode occurs in v0.1
- **THEN** it is resolved locally without a network request

