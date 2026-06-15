## Context

Per §5.3, an embedding should carry its space as part of its type so the compiler can reject cross-space comparison. This is phantom typing / units-of-measure applied to vectors — a different mechanism from the capability/effect discipline used by memory and familiar, so this change is independent of `add-capability-effects`. It is the most defensible of the remaining primitives and a good pattern-setter.

## Goals / Non-Goals

**Goals:**
- `embedding` values typed by space; space derived from the producing oracle.
- `oracle.embed` produces a space-tagged embedding with provenance.
- Same-space `similarity`/`nearest`; cross-space comparison = compile error.

**Non-Goals:**
- Storage/retrieval/indexing (memory's job); ANN performance.
- Cross-space projection or implicit bridging (intentionally absent).
- Any semantic guarantee about embedding meaning or retrieval relevance (§8).

## Decisions

### D1: Space is a type-level tag (phantom type)
An `embedding` has type `embedding@S` where `S` is a space identity. `S` is part of the static type, carried at compile time and erased at runtime except as provenance. **Why:** lets the compiler reject `similarity(a: embedding@S1, b: embedding@S2)` structurally. *Alternative:* runtime space check — rejected, it's exactly the silent-until-runtime failure the paper targets.

### D2: Space identity = the producing model id (v0.x)
The space `S` is identified by the oracle's model id. `triage.embed(x)` yields `embedding@"support-reasoner-v3"`. **Why:** simplest defensible identity. **Trade-off / §5.5 echo:** this is coarse — two model versions may not share a space, and two different models occasionally might. Documented as the honest extension point (space = model + version + pooling later). *Alternative:* nominal space declarations decoupled from models — deferred.

### D3: Same-space-only operations, no implicit projection
`similarity` and `nearest` are defined only for matching spaces. There is deliberately no implicit cross-space conversion. **Why:** an implicit bridge would reintroduce the bug as a silent coercion. If cross-space comparison is ever wanted it must be an explicit, named, lossy operation (out of scope here).

### D4: Reuse the provenance seam
The space tag and the oracle id/seed are recorded in the same provenance representation bootstrap attached to `Inferred<T>`. **Why:** one provenance model across the language; embeddings and inferred values share lineage shape.

### D5: Vector math is deterministic and local
Similarity is ordinary deterministic float math; `nearest` is an exact (non-approximate) scan in v0.x. **Why:** determinism for tests; performance is explicitly out of scope.

## Risks / Trade-offs

- **Space = model id is too coarse (§5.5)** → Accept for v0.x; isolate space identity behind one definition so refining it to model+version+pooling later does not move call sites.
- **Float determinism across platforms** → Keep similarity math simple and documented; pin ordering/tie-breaking in `nearest` so results are reproducible.
- **Users want cross-space comparison and feel blocked** → That friction is the point (the bug made unrepresentable); document why no implicit bridge exists.
- **Treating "nearest" as relevance (§8)** → Compiler guarantees same-space, not relevance; state it in docs.

## Open Questions

- Space identity granularity for v0.x: model id only (lean), or model id + version? Resolve before tasks.
- `nearest` candidate source: a plain list of embeddings here, with memory providing the indexed source later — confirm the signature stays stable when memory consumes it.
- Should `embedding` literals exist at all, or only embeddings produced via `oracle.embed`? Lean: only via `oracle.embed` (no way to forge a space), which strengthens the guarantee.
