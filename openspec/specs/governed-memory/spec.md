# governed-memory Specification

## Purpose
Make `memory` a declared, governed runtime resource whose `scope` is bound to a
capability (per capability-effects), so a read or write outside its scope is a
compile-time error — the §5.2 cross-tenant leak that will not compile. Retention
and audit are runtime-enforced (deterministic in-memory store, logical clock).
The green check guarantees scope adherence only, never that retained data is
correct, audit complete, or retrieval relevant (§8). Memory is interpreter-only
in v0.1.

## Requirements
### Requirement: Memory is a declared governed resource
A `memory` SHALL be declared with a `scope`, and MAY declare a `retention`, a `retrieval` policy, and an `audit` setting. The declaration SHALL produce a typed memory resource whose operations are governed by these settings.

#### Scenario: Declare a governed memory
- **WHEN** a program declares `memory tickets { scope tenant, retention 24 months, retrieval recency, audit required }`
- **THEN** the checker records a memory resource `tickets` carrying those governance settings

#### Scenario: Missing required governance is rejected
- **WHEN** a `memory` is declared without a `scope`
- **THEN** `witch check` reports an error requiring a scope declaration

### Requirement: Scope is bound to a capability
Each memory's `scope` SHALL correspond to a capability (per capability-effects). Reading from or writing to the memory SHALL require that scope capability, which is granted only by entering the scope (`within <scope>`).

#### Scenario: In-scope access is granted
- **WHEN** code reads `tickets` inside a `within tenant` region that grants the `tenant` scope
- **THEN** the access type-checks

#### Scenario: Scope grant does not leak
- **WHEN** code reads `tickets` after the `within tenant` region has ended
- **THEN** the type checker reports a missing scope capability

### Requirement: Out-of-scope access is a compile-time error
A read or write to a scoped memory performed without the matching scope capability in context SHALL be a compile-time error, naming the memory and the missing scope. The program SHALL NOT run.

#### Scenario: Cross-tenant read will not compile
- **WHEN** code attempts to read `tickets` without holding its `tenant` scope
- **THEN** `witch check` reports an out-of-scope error naming `tickets` and the scope, and exits non-zero

### Requirement: Scoped recency retrieval
The memory SHALL provide scoped retrieval — `recent(k)` (and the `nearest(query, k)` signature) — returning entries from within the granted scope ordered by recency. Recency/exact retrieval SHALL be available without embeddings. Semantic retrieval (a query `embedding@S` compared against stored embeddings of the same space, per typed-embedding) is composed in the triage-flagship change; the retrieval signature is kept stable so it can be added there without reshaping callers.

#### Scenario: Recency retrieval within scope
- **WHEN** `tickets.recent(5)` is evaluated inside the granting scope
- **THEN** it returns up to 5 non-expired entries ordered newest-first

### Requirement: Retention and audit are runtime-enforced
The runtime SHALL enforce retention by making entries older than the declared retention unretrievable, and SHALL enforce `audit required` by producing an audit record for each governed access. These are runtime guarantees; the compiler SHALL verify only that retention and audit are declared, not that outcomes are correct.

#### Scenario: Expired entries are not retrieved
- **WHEN** an entry older than the declared retention exists and a retrieval runs
- **THEN** the expired entry is not returned

#### Scenario: Audited access produces a record
- **WHEN** a governed access occurs on a memory declared `audit required`
- **THEN** the runtime produces an audit record for that access

### Requirement: Structural guarantee only
A successful check SHALL guarantee scope adherence, never that retained data is correct, that audit is complete, or that retrieved context is relevant (§8).

#### Scenario: Green check is not a governance-correctness claim
- **WHEN** a memory-using program type-checks
- **THEN** tool output asserts only scope/structural correctness, not retention/audit/relevance correctness
