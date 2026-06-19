## ADDED Requirements

### Requirement: Record literal expressions
The parser SHALL accept record literal expressions of the form `{ field: expr, ... }` in expression positions, including as the `fallback` expression of a `divine` block.

#### Scenario: Parse a record literal in fallback
- **WHEN** the parser reads `fallback { narration: "idle", outcome: Nothing, danger: 0 }`
- **THEN** it produces a record literal expression with three fields

#### Scenario: Record literal disambiguated from blocks
- **WHEN** `{` appears after `fallback` and is followed by an identifier and `:`
- **THEN** it is parsed as a record literal, not a statement block
