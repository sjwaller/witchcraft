## Context

§5.2 wants memory to be a governed resource the compiler and runtime can police: scope violations become type errors, and retention/audit are enforced rather than hoped for. The scope guarantee is the same capability discipline as familiar's permits, so this change *consumes* `add-capability-effects` rather than inventing its own checker. Retention and audit are genuinely runtime concerns, and the design is careful to say so (§8): not everything about governed memory is statically checkable, and we must not pretend it is.

## Goals / Non-Goals

**Goals:**
- `memory` declaration with `scope`, `retention`, `retrieval`, `audit`.
- Scope bound to a capability; out-of-scope read/write = compile error.
- Scoped retrieval `nearest(q, k) within <scope>`; **recency/exact only** in this change (semantic retrieval deferred to `integrate-triage-flagship`).
- Runtime retention (expired entries unretrievable) and audit (accesses logged), deterministic.

**Non-Goals:**
- Durable/external storage; ANN/performance; nested scopes; cross-scope sharing.
- Static guarantees about retention/audit beyond their being *declared* and runtime-enforced.
- Any guarantee that retrieved context is relevant (§8).

## Decisions

### D1: Scope is a capability (consume `add-capability-effects`)
A `memory` declared `scope tenant` makes its read/write operations require the capability `scope(tenant)`. Entering a scope (e.g. `within customer`) grants `scope(tenant)` for that region. **Why:** reuses the shared substrate; the cross-tenant-leak guarantee falls out of the generic ungranted-capability error. *Alternative:* a memory-specific scope checker — rejected (the drift this whole substrate exists to prevent).

### D2: The store is a deterministic in-memory runtime resource
v0.x ships an in-memory store with a logical clock for retention and an in-memory audit log. **Why:** determinism for tests; durability is out of scope and orthogonal. Real backends slot behind the store interface later, like the decoder seam.

### D3: Retention and audit are runtime-enforced — and that boundary is explicit
The compiler checks that retention/audit are *declared* and that accesses are *in scope*. It does **not** statically guarantee retention correctness or audit completeness — those are runtime behaviours. **Why (§8):** honesty. Expired entries are filtered at retrieval time; audited accesses emit a record at access time.

### D4: Recency/exact only here; semantic retrieval deferred to the flagship (decided)
`nearest` supports **recency/exact retrieval** in this change, with **no embedding dependency**. The paper's `retrieval semantic` half — a query `embedding@S` compared against stored `embedding@S` (same-space, from `typed-embedding`) — is **deferred and composed in `integrate-triage-flagship`**; it is **not** a requirement of `add-memory-primitive`. **Why:** keeps this change shippable and independently verifiable without `add-embedding-primitive`; the flagship is where memory and embeddings meet. The `nearest` signature is designed to stay stable when semantic retrieval is added there.

### D5: Single-level scope for v0.x
One scope dimension (e.g. tenant). Nested scopes (tenant → user → session) are deferred. **Why:** smallest defensible version of the guarantee; nesting is additive once the capability substrate proves out.

## Risks / Trade-offs

- **People read the green build as "data is governed" (§8/§10)** → It means *in-scope and declared*, not *retained/audited correctly*. State loudly in diagnostics and docs.
- **Semantic retrieval coupling to embeddings sequencing** → Resolved by deferring semantic retrieval entirely to `integrate-triage-flagship`; this change ships recency/exact only and does not depend on `add-embedding-primitive`. Keep the `nearest` signature stable so the flagship can add semantic retrieval without reshaping it.
- **Scope-as-capability proves too weak (higher-order memory handles)** → Same mitigation as the substrate: narrow interface, revisit toward effect rows only if needed.
- **In-memory store mistaken for production storage** → Documented as v0.x; behind a store interface for later backends.

## Open Questions

- Grant syntax and value-bound capability identity are resolved in `add-capability-effects` (D1/D4); this change consumes that resolution.
- Does `audit required` change the *type* of an access (forcing a discharge-like acknowledgement), or is it purely a runtime log? Lean: runtime log for v0.x; flag the typed-audit option.
- Retention units/clock model for deterministic tests (logical ticks vs wall-clock-with-injected-time). Lean: injectable logical clock.
