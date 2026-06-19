## ADDED Requirements

### Requirement: Record literal type checking
When a record literal appears in a context expecting a record type, the type checker SHALL verify that every required field is present, no unknown fields are added, and each field expression is assignable to the declared field type. For a `divine` fallback, the expected type SHALL be the declared output type of that `divine`.

#### Scenario: Fallback must match divine output type
- **WHEN** `divine t: Turn ... fallback { narration: "x", outcome: Nothing, danger: 1 }` and `Turn` requires field `exit`
- **THEN** `witch check` reports a type error naming the missing field

#### Scenario: Well-formed fallback passes
- **WHEN** the fallback literal includes all fields of `Turn` with compatible types
- **THEN** `witch check` accepts the program
