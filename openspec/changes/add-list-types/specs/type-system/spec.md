## ADDED Requirements

### Requirement: List type resolution
The type checker SHALL resolve `list of T` to a list type with element `T` and no length bound, and `list of lo..hi of T` to a list type with element `T` and inclusive length bounds `[lo, hi]`. List literals `[e1, e2, ...]` SHALL type-check against `list of T` when each element is assignable to `T`. When length bounds are present, the literal length SHALL be checked against the bounds.

#### Scenario: List literal matches element type
- **WHEN** `[North, West]` is checked against `list of 0..4 of one_of { North, South, East, West }`
- **THEN** the expression is well-typed

#### Scenario: List literal exceeds upper bound
- **WHEN** a list literal has five elements checked against `list of 0..4 of glyph`
- **THEN** `witch check` reports a length bound error

#### Scenario: Display bounded list type
- **WHEN** a diagnostic prints `list of 0..4 of one_of { ... }`
- **THEN** the displayed type includes both bounds and element type
