## ADDED Requirements

### Requirement: Memory is a declared governed resource
A `memory` SHALL be declared with a `scope`, a `retention`, a `retrieval` policy, and an `audit` setting. The declaration SHALL produce a typed memory resource whose operations are governed by these settings.

#### Scenario: Declare a governed memory
- **WHEN** a program declares `memory tickets:` with `scope tenant`, `retention 24 months`, `retrieval semantic + recency`, `audit required`
- **THEN** the checker records a memory resource `tickets` carrying those governance settings

#### Scenario: Missing required governance is rejected
- **WHEN** a `memory` is declared without a `scope`
- **THEN** `witch check` reports an error requiring a scope declaration

### Requirement: Scope is bound to a capability
Each memory's `scope` SHALL correspond to a capability (per `capability-effects`). Reading from or writing to the memory SHALL require that scope capability, which is granted only by entering the scope (e.g. `within <value>`).

#### Scenario: In-scope access is granted
- **WHEN** code reads `tickets` inside a `within customer` region that grants the `tenant` scope
- **THEN** the access type-checks

#### Scenario: Scope grant does not leak
- **WHEN** code reads `tickets` after the `within customer` region has ended
- **THEN** the type checker reports a missing scope capability

### Requirement: Out-of-scope access is a compile-time error
A read or write to a scoped memory performed without the matching scope capability in context SHALL be a compile-time error, naming the memory and the missing scope. The program SHALL NOT run.

#### Scenario: Cross-tenant read will not compile
- **WHEN** code serving tenant B attempts to read tenant A's `tickets` without holding A's scope
- **THEN** `witch check` reports an out-of-scope error naming `tickets` and the scope, and exits non-zero

### Requirement: Scoped retrieval
The memory SHALL provide `nearest(query, k) within <scope>` retrieval. Recency/exact retrieval SHALL be available without embeddings. Semantic retrieval SHALL compare a query `embedding@S` against stored embeddings of the same space `S` (per typed-embedding); a cross-space semantic query SHALL be a compile-time error.

#### Scenario: Recency retrieval within scope
- **WHEN** `tickets.nearest(criteria, 5) within customer` is evaluated with recency retrieval
- **THEN** it returns up to 5 entries from `customer`'s scope ordered by the policy

#### Scenario: Semantic retrieval requires same-space query
- **WHEN** a semantic `nearest` is given a query embedding whose space differs from the stored embeddings' space
- **THEN** `witch check` reports a cross-space error and the program does not run

### Requirement: Retention and audit are runtime-enforced
The runtime SHALL enforce retention by making entries older than the declared retention unretrievable, and SHALL enforce `audit required` by producing an audit record for each governed access. These are runtime guarantees; the compiler SHALL verify only that retention and audit are declared, not that outcomes are correct.

#### Scenario: Expired entries are not retrieved
- **WHEN** an entry older than the declared retention exists and a retrieval runs
- **THEN** the expired entry is not returned

#### Scenario: Audited access produces a record
- **WHEN** a governed access occurs on a memory declared `audit required`
- **THEN** the runtime produces an audit record for that access

### Requirement: Structural guarantee only
A successful check SHALL guarantee scope adherence (and same-space semantic queries), never that retained data is correct, that audit is complete, or that retrieved context is relevant (§8).

#### Scenario: Green check is not a governance-correctness claim
- **WHEN** a memory-using program type-checks
- **THEN** tool output asserts only scope/structural correctness, not retention/audit/relevance correctness
