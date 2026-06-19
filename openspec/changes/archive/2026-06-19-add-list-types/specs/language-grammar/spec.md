## ADDED Requirements

### Requirement: List type declarations
The parser SHALL accept list types in type declarations:
- `list of <Type>` for a homogeneous list with no length bound, and
- `list of <lo>..<hi> of <Type>` for a list whose length is bounded inclusively between `<lo>` and `<hi>`.

#### Scenario: Parse unbounded list type
- **WHEN** the parser reads `exits: list of glyph`
- **THEN** the AST records a list type with element `glyph` and no length bound

#### Scenario: Parse bounded list type
- **WHEN** the parser reads `exits: list of 0..4 of one_of { North, South, East, West }`
- **THEN** the AST records a list type with lower bound 0, upper bound 4, and the given element type

#### Scenario: Reject bare list of without element
- **WHEN** the parser reads `list of` with no following type
- **THEN** a syntax error is reported
