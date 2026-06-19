# type-system Specification

## Purpose
TBD - created by archiving change bootstrap-language-core. Update Purpose after archive.
## Requirements
### Requirement: Records, variants, and refinement types
The type checker SHALL support record types, sum/variant types (`one_of`), and refinement types over `spark` (e.g. `spark in 0..10`). A value SHALL type-check against a refinement only if it provably satisfies the bound; a variant value SHALL type-check only against a sum type that declares that variant.

#### Scenario: Refinement out of range is rejected
- **WHEN** a program assigns the literal `11` to a binding declared `spark in 0..10`
- **THEN** `witch check` reports a type error stating the value is outside the refinement bound

#### Scenario: Unknown variant is rejected
- **WHEN** a program constructs a variant not declared by the target sum type
- **THEN** `witch check` reports a type error naming the unknown variant

### Requirement: Inferred values carry confidence and provenance
The type system SHALL represent the result of inference as a distinct *inferred* type (written `Inferred<T>` in this spec) that wraps an underlying type `T` together with a confidence value and a provenance record. An `Inferred<T>` SHALL NOT be assignable to a context expecting a plain `T`.

#### Scenario: Inferred is not a plain value
- **WHEN** a program uses an `Inferred<Disposition>` where a plain `Disposition` is required, without discharging it
- **THEN** `witch check` reports a type error indicating the inferred value must be discharged first

#### Scenario: Provenance is part of the value
- **WHEN** an inferred value is produced
- **THEN** it carries a provenance record identifying at least the originating oracle and the inputs/seed used

### Requirement: The discharge rule
An `Inferred<T>` SHALL be narrowed to `T` only through a confidence discharge (`with confidence >= θ`). On the success path the value SHALL have type `T`; on the failure path control SHALL transfer to the declared `fallback`. Using an inferred value authoritatively without discharge SHALL be a compile-time error.

#### Scenario: Undischarged authoritative use fails to compile
- **WHEN** a program reads a field of, or `enact`s, an inferred value that has not passed a confidence gate
- **THEN** `witch check` reports a discharge error and the program does not run

#### Scenario: Discharged value is usable as T
- **WHEN** an `Inferred<Disposition>` passes `with confidence >= 0.80`
- **THEN** on the success path it is typed as a plain `Disposition` and its fields may be used

### Requirement: enact exhaustiveness
`enact` over a value whose type is a sum/variant type SHALL require that the program (and the runtime's action dispatch) account for exactly the declared variants. A missing or extra variant SHALL be a compile-time error.

#### Scenario: Non-exhaustive enact fails to compile
- **WHEN** an action type declares variants `Draft`, `Escalate`, `AskClarify` and the program's handling omits `AskClarify`
- **THEN** `witch check` reports a non-exhaustiveness error naming the missing variant

#### Scenario: Only declared variants are reachable
- **WHEN** `enact` dispatches on a discharged action value
- **THEN** only the declared variants are reachable as outcomes; no other shape can be enacted

### Requirement: Structural guarantees are not semantic guarantees
The type checker SHALL verify only structural properties (discharge, exhaustiveness, refinement bounds, variant validity). A successful `witch check` SHALL NOT be represented, in diagnostics or documentation, as a guarantee that an inferred value is correct, calibrated, or true.

#### Scenario: Green check is not a correctness claim
- **WHEN** `witch check` succeeds for a program containing `divine`
- **THEN** the tool's output does not assert the inferred result is correct, only that the program is structurally well-formed

### Requirement: Record literal type checking
When a record literal appears in a context expecting a record type, the type checker SHALL verify that every required field is present, no unknown fields are added, and each field expression is assignable to the declared field type. For a `divine` fallback, the expected type SHALL be the declared output type of that `divine`.

#### Scenario: Fallback must match divine output type
- **WHEN** `divine t: Turn ... fallback { narration: "x", outcome: Nothing, danger: 1 }` and `Turn` requires field `exit`
- **THEN** `witch check` reports a type error naming the missing field

#### Scenario: Well-formed fallback passes
- **WHEN** the fallback literal includes all fields of `Turn` with compatible types
- **THEN** `witch check` accepts the program

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

