## ADDED Requirements

### Requirement: Operations may require named capabilities
The type system SHALL allow an operation (a built-in or a `fn`) to declare that it requires one or more named capabilities. A capability SHALL be identified by a structured name (a kind plus optional parameters, e.g. `scope(tenant)`, `permit(escalate)`) so that other features can mint their own capability kinds against this mechanism.

#### Scenario: Function declares a required capability
- **WHEN** a `fn` is declared with `requires permit(escalate)` and its body performs the escalate operation
- **THEN** the type checker records that the function requires `permit(escalate)` and surfaces that requirement as part of its signature

#### Scenario: Capability names are structured
- **WHEN** two capabilities `scope(tenant)` and `scope(user)` are declared
- **THEN** the checker treats them as distinct capabilities (kind plus parameter), not as one

### Requirement: Capability grants over a lexical region
The language SHALL provide a way to grant one or more capabilities to the code within a bounded lexical region. Within that region the granted capabilities SHALL be present in the active capability context; outside it they SHALL NOT.

#### Scenario: Grant makes a capability available within the region
- **WHEN** code performs an operation requiring `permit(escalate)` inside a region that grants `permit(escalate)`
- **THEN** the operation type-checks

#### Scenario: Grant does not leak past its region
- **WHEN** an operation requiring `permit(escalate)` appears after the granting region has ended
- **THEN** the type checker reports a missing-capability error

### Requirement: Ungranted required capability is a compile-time error
Performing an operation whose required capability is not present in the active context SHALL be a compile-time error, reported with the offending operation and the missing capability. The program SHALL NOT run.

#### Scenario: Missing capability fails the check
- **WHEN** `witch check` encounters an operation requiring `scope(tenant)` with no `scope(tenant)` granted in context
- **THEN** it reports a missing-capability error naming the operation and `scope(tenant)`, and exits non-zero

#### Scenario: Granted capability passes the check
- **WHEN** the same operation runs inside a context granting `scope(tenant)`
- **THEN** `witch check` reports no capability error for that operation

### Requirement: Requirements propagate transitively to callers
When a `fn` requires a capability, calling it SHALL itself require that capability: the caller MUST either be within a context that grants it or declare the same requirement. Calling a capability-requiring function from a context that neither grants nor declares the capability SHALL be a compile-time error.

#### Scenario: Caller without the capability fails
- **WHEN** a function `a()` requires `permit(escalate)` and is called from `b()` which neither grants nor declares `permit(escalate)`
- **THEN** the type checker reports a missing-capability error at the call site in `b()`

#### Scenario: Caller that re-declares the requirement passes
- **WHEN** `b()` is declared `requires permit(escalate)` and calls `a()` which requires the same
- **THEN** the call type-checks, and `b()`'s own callers inherit the requirement

### Requirement: Capability checking is structural, not semantic
A successful capability check SHALL be represented only as "the operation is permitted in this context," never as a guarantee that performing it is correct, safe in outcome, or wise. Diagnostics and documentation SHALL NOT imply semantic correctness from capability adherence.

#### Scenario: Passing check is not a correctness claim
- **WHEN** capability checking succeeds for a program
- **THEN** tool output asserts only that required capabilities are granted, not that the capability-bearing operations behave correctly
