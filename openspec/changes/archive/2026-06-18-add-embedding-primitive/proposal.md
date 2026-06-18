## Why

Embeddings are already the basic unit of model-mediated software — the representation in which similarity, retrieval and clustering happen — yet no mainstream language types them; they are arrays of floats the compiler cannot distinguish from any other array (§5.3). The consequence is that the most common embedding bug, comparing vectors from incompatible models or spaces, is invisible until it produces silently wrong results. The native move is to make an `embedding` carry its *vector space* as part of its type, so the compiler refuses a cross-space comparison the way a strong type system refuses to add a length to a duration. The paper calls this "a small, concrete example of nativeness earning its place," and it is the most clearly-defensible of the three remaining primitives — it passes every point of the §4 discriminator.

## What Changes

- Introduce `embedding` as a first-class typed value that **carries its space** in its type (phantom-typed, e.g. `embedding@<space>`), where the space is derived from the producing oracle/model.
- Add `oracle.embed(<glyph>)` producing an `embedding@<that-oracle's-space>`, contributing the space (and the oracle id/seed) to the value's provenance — reusing bootstrap's provenance seam.
- Add space-aware operations: `similarity(a, b)` and `nearest(query, candidates, k)`, defined **only** between embeddings of the same space.
- Enforce the **native guarantee**: a similarity/nearest operation between embeddings of different spaces is a **compile-time error**, with a diagnostic naming both spaces.

**Non-goals (deferred):** the `memory` primitive's storage/retrieval (separate change; memory will *consume* embeddings for semantic retrieval); vector persistence; approximate-nearest-neighbour indexing/performance; cross-space *projection* (deliberately not provided — there is no implicit bridge between spaces). This change adds **structural** typing only; it cannot verify an embedding is *semantically meaningful* or that "nearest" is actually *relevant* (§8).

## Capabilities

### New Capabilities
- `typed-embedding`: the `embedding` type carrying its space, `oracle.embed`, same-space `similarity`/`nearest`, and the compile-time rejection of cross-space comparison. Includes the `oracle.embed` addition (kept here to keep the change self-contained rather than a delta against bootstrap's `model-as-value`).

### Modified Capabilities
<!-- None as delta specs (bootstrap not yet archived). Builds on bootstrap's model-as-value and type-system conceptually; see Impact. -->

## Impact

- Builds on `bootstrap-language-core` (`model-as-value` for the oracle, `type-system` for the space tag, `host-runtime` for values). Independent of `add-capability-effects`, `add-memory-primitive`, and `add-familiar-primitive`.
- **Unblocks semantic retrieval** in `add-memory-primitive` (whose `retrieval semantic` half needs typed embeddings) and is required by `integrate-triage-flagship`.
- Reuses the bootstrap provenance representation for the space tag; no new runtime subsystem beyond vector storage-free similarity math.
- Build order: `bootstrap-language-core` must be implemented and archived before this change's specs can become delta-aware; this change adds new capabilities until then.
