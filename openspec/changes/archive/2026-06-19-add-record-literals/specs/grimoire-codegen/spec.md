## ADDED Requirements

### Requirement: Lower record literals
The code generator SHALL lower record literals to native code that constructs record values field-by-field, with the same layout as decoder-produced records.

#### Scenario: Compiled record literal matches interpreter
- **WHEN** a program returns `{ n: 42 }` from a `define`
- **THEN** the compiled artifact returns the same record value as `witch run` under the same seed
