## ADDED Requirements

### Requirement: Divine output lists must be bounded
A field of a `divine` output type MAY be a list type only when declared with explicit inclusive length bounds (`list of lo..hi of T`). The type checker SHALL reject a `divine` output type containing an unbounded list type.

#### Scenario: Bounded list field on divine output is accepted
- **WHEN** `divine t: Turn` where `Turn` contains `exits: list of 0..4 of one_of { North, South, East, West }`
- **THEN** `witch check` accepts the program pending other rules

#### Scenario: Unbounded list on divine output is rejected
- **WHEN** a `divine` output record field is `items: list of glyph`
- **THEN** `witch check` reports an error that inference output lists require bounds
