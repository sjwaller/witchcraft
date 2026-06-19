## ADDED Requirements

### Requirement: Bounded list grammar production
The runtime SHALL compile a bounded list type `list of lo..hi of T` into a list grammar production that admits only lists whose length is between `lo` and `hi` inclusive and whose every element inhabits the grammar compiled from `T`. An unbounded list type (`list of T` without bounds) SHALL NOT compile as a `divine` output field.

#### Scenario: Bounded list grammar admits only in-range lengths
- **WHEN** `list of 0..2 of one_of { A, B }` is compiled to a grammar
- **THEN** generated values are `[]`, `[A]`, `[B]`, `[A,A]`, etc. with at most two elements

#### Scenario: Unbounded list rejected on divine output
- **WHEN** a `divine` output record field is declared `list of glyph` without bounds
- **THEN** `witch check` reports that unbounded lists cannot be inference output types

### Requirement: List generation is by construction
Decoders (Mock, llama GBNF, litmus-safe frontier) SHALL generate list values by constrained production, not by emitting free text followed by validation. For the Mock engine, choosing a length outside `[lo, hi]` or an element outside the element grammar SHALL be unreachable.

#### Scenario: Illegal length is unreachable
- **WHEN** the Mock decoder generates against `list of 0..4 of one_of { North, South }`
- **THEN** no generated value has more than four elements

#### Scenario: Litmus distinguishes weakened bound
- **WHEN** the same seed generates against `list of 0..1 of one_of { A, B }` and against a weakened grammar with upper bound 4
- **THEN** the set of reachable list lengths differs between the two grammars

## MODIFIED Requirements

### Requirement: Output types compile to a generation grammar
The runtime SHALL compile an inference output type into a generation grammar: a record type into an ordered field grammar; a sum type `one_of { ... }` into an alternation over exactly its declared variants; a refinement `spark in a..b` into a numeric production bounded to that range; a `glyph` into a bounded text production; a bounded list type `list of lo..hi of T` into a list production as defined in bounded-list-generation. The compiled grammar SHALL admit exactly the values that inhabit the type.

#### Scenario: Record with list field compiles
- **WHEN** a record type includes `exits: list of 0..4 of one_of { North, South }`
- **THEN** the record grammar generates the `exits` field using the list production for that bound and element type

#### Scenario: Variant type compiles to a closed alternation
- **WHEN** the type `one_of { Draft, Escalate, AskClarify }` is compiled to a grammar
- **THEN** the grammar admits exactly those three variants and no others
