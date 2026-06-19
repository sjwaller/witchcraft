# list-types Specification

## Purpose
Surface syntax for homogeneous list types with optional length bounds on the host side.

## Requirements
### Requirement: Host-side list types
List types SHALL be available in type declarations for host-side bindings and record fields. Unbounded `list of T` SHALL NOT be accepted as a top-level `divine` output type until constrained list generation is implemented.

#### Scenario: Unbounded list rejected as divine output
- **WHEN** a program declares `divine x: list of glyph from (...) using ...`
- **THEN** `witch check` reports that the type cannot be a `divine` output type
