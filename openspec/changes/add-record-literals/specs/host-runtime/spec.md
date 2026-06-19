## ADDED Requirements

### Requirement: Evaluate record literals
The interpreter SHALL evaluate a record literal by evaluating each field expression and constructing a record value whose fields are those results.

#### Scenario: Construct a record value
- **WHEN** `{ a: 1, b: "x" }` is evaluated
- **THEN** the result is a record value with fields `a = 1` and `b = "x"`
