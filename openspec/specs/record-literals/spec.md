# record-literals Specification

## Purpose
Host-side record literal expressions `{ field: expr }` for constructing plain record values, especially as typed `divine` fallbacks.

## Requirements
### Requirement: Record literal expressions construct plain values
Record literals SHALL construct plain record values, not objects with methods. Behaviour remains in `define` functions; the language is not object-oriented.

#### Scenario: Record literal is a value expression
- **WHEN** a program binds `let p = { x: 1, y: 2 }`
- **THEN** `p` is a record value whose fields may be read with dot access
