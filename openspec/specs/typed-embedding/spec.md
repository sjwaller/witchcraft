# typed-embedding Specification

## Purpose
Make an `embedding` a first-class value typed by its vector space
(`embedding@space`, where the space is the producing oracle's model id), so the
compiler rejects cross-space comparison structurally. `oracle.embed` produces a
space-tagged embedding with provenance; `similarity`/`nearest` are defined only
within a space. The guarantee is structural — same-space, never relevance (§8).
Embeddings are interpreter-only in v0.1.

## Requirements
### Requirement: Embeddings carry their space in their type
An `embedding` SHALL be a first-class value whose type includes a space identity (written `embedding@S` in this spec). The space SHALL be part of the static type, not merely a runtime field. Embeddings of different spaces SHALL have distinct, non-interchangeable types.

#### Scenario: Embeddings of different spaces are different types
- **WHEN** one embedding has type `embedding@A` and another `embedding@B`
- **THEN** the type checker treats them as incompatible types and does not allow one where the other is required

#### Scenario: Space is static, not just runtime
- **WHEN** `witch check` runs on a program using embeddings
- **THEN** the space of each embedding is known at check time without executing the program

### Requirement: oracle.embed produces a space-tagged embedding
`oracle.embed(<glyph>)` SHALL produce an `embedding@S` where `S` is the space of the producing oracle. The value SHALL record its space, originating oracle id, and decoder seed in its provenance (reusing the bootstrap provenance representation).

#### Scenario: Embed yields the oracle's space
- **WHEN** an oracle `triage` (model `support-reasoner-v3`) evaluates `triage.embed("payment failed")`
- **THEN** the result has type `embedding@support-reasoner-v3` and provenance naming `triage`

#### Scenario: Embeddings cannot be forged without a space
- **WHEN** a program attempts to obtain an embedding other than through an oracle's `embed`
- **THEN** no such operation exists in the surface (every embedding has a space derived from its producing oracle)

### Requirement: Same-space similarity and nearest
`similarity(a, b)` and `nearest(query, candidates, k)` SHALL be defined only when all participating embeddings share the same space. Within one space, `similarity` SHALL return a deterministic scalar and `nearest` SHALL return the `k` closest candidates with deterministic tie-breaking.

#### Scenario: Same-space similarity type-checks and is deterministic
- **WHEN** `similarity(a, b)` is called with `a, b : embedding@S`
- **THEN** it type-checks and returns the same scalar on repeated runs with the same inputs

#### Scenario: Nearest returns k results within a space
- **WHEN** `nearest(q, candidates, 5)` is called with all embeddings in space `S`
- **THEN** it returns the 5 closest candidates with deterministic ordering

### Requirement: Cross-space comparison is a compile-time error
A `similarity` or `nearest` operation between embeddings of different spaces SHALL be a compile-time error, naming both spaces. There SHALL be no implicit cross-space projection or coercion.

#### Scenario: Cross-space similarity fails to compile
- **WHEN** `similarity(a, b)` is called with `a : embedding@A` and `b : embedding@B`
- **THEN** `witch check` reports a cross-space error naming `A` and `B`, and the program does not run

#### Scenario: No implicit bridge between spaces
- **WHEN** a program relies on an embedding of space `A` being usable where space `B` is required
- **THEN** the type checker rejects it; no automatic conversion is applied

### Requirement: Structural guarantee only
A successful check SHALL guarantee only that compared embeddings share a space, never that an embedding is semantically meaningful or that a `nearest` result is relevant.

#### Scenario: Same-space pass is not a relevance claim
- **WHEN** a same-space `nearest` type-checks
- **THEN** tool output does not assert the returned neighbours are semantically relevant, only that the comparison is well-typed
