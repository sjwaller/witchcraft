# bounded-familiar Specification

## Purpose
Provide `familiar` as a named, bounded composite (explicitly not a primitive,
§5.5) whose `permits` set is the elevation-worthy, checkable boundary: the body is
granted exactly those capabilities (per capability-effects) and an action outside
them will not compile. v0.1 familiars are single-pass and deterministic (the §10
firebreak): no free-running loop or scheduler. The green check guarantees permit
adherence and bounded structure only, never that the agent is sound, well-behaved,
or terminating (§8/§10). Familiars are interpreter-only in v0.1.

## Requirements
### Requirement: Familiar is a bounded composite with declared permits
A `familiar` SHALL be a named construct declaring its inputs, a `permits` set, and a bounded body. It SHALL be defined and documented as a composite (built from oracle/divine/enact and, where present, memory/embedding), not as a new primitive. The `permits` set SHALL be the elevation-worthy, checkable boundary.

#### Scenario: Declare a bounded familiar
- **WHEN** a program declares `familiar support_triage(msg) permits { invoke triage, escalate } ...`
- **THEN** the checker records a familiar with those inputs and the permit set `{ invoke triage, escalate }`

#### Scenario: Permits are surfaced for legibility
- **WHEN** the checker processes a familiar
- **THEN** its declared permits define the capabilities active in its body and are the surface against which actions are checked

### Requirement: Permits are capabilities granted to the body
A familiar's `permits` SHALL grant exactly the corresponding capabilities (per capability-effects) to its body, and no others. Actions within the body that require a capability SHALL type-check only if that capability is in `permits`.

#### Scenario: Permitted action type-checks
- **WHEN** a familiar permitting `invoke triage` performs a `divine ... using triage`
- **THEN** the action type-checks

#### Scenario: Body cannot exceed its permits
- **WHEN** a familiar's body performs an action requiring a capability outside its `permits`
- **THEN** the type checker reports a permit-violation error

### Requirement: Acting outside permits is a compile-time error
A familiar SHALL NOT perform an action outside its declared `permits`. An attempt to do so SHALL be a compile-time error naming the familiar and the disallowed action. The program SHALL NOT run.

#### Scenario: Out-of-permit action will not compile
- **WHEN** a familiar permitting only `{ invoke triage }` performs a `delete` action
- **THEN** `witch check` reports a permit-violation error naming the familiar and `delete`, and exits non-zero

### Requirement: Bounded, deterministic execution
A familiar SHALL execute as a single-pass, deterministic, bounded procedure in this version: no free-running loop, no scheduling, and no concurrency. A familiar body containing an unbounded loop SHALL be a compile-time error.

#### Scenario: Single-pass familiar runs deterministically
- **WHEN** a bounded familiar is run twice with the same inputs and decoder seed
- **THEN** it produces identical output and terminates

#### Scenario: Unbounded iteration is rejected
- **WHEN** a familiar body contains an unbounded loop
- **THEN** `witch check` reports a missing-bound error

### Requirement: Composition reuses bootstrap semantics
A familiar's inference SHALL use `divine` (with its confidence discharge) and its actions SHALL flow through `enact` (exhaustive over the action variant type). The familiar SHALL NOT introduce a parallel inference or action mechanism.

#### Scenario: Familiar uses divine and enact
- **WHEN** a familiar resolves a decision via `divine` and executes it via `enact`
- **THEN** the discharge rule and `enact` exhaustiveness from bootstrap apply unchanged

### Requirement: Structural guarantee only
A successful check SHALL guarantee that the familiar stays within its permits and has bounded iteration, never that its plan is sound, that it behaves well, or that it terminates in practice.

#### Scenario: Green check is not a behaviour guarantee
- **WHEN** a familiar type-checks
- **THEN** tool output asserts only permit adherence and structural bounds, not soundness or good behaviour of the agent
