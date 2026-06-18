## Why

Memory in model-mediated systems is today "an accidental architecture: a vector database bolted to the side of an application, accessed through queries the language does not understand" (§5.2). As a library, governance — scope, retention, audit — "depends entirely on discipline, and is absent exactly when it matters most." The native move is to make `memory` a *governed runtime resource* whose declared scope is part of its type, so that a read outside that scope is a type error rather than a silent cross-tenant data leak. This is the headline guarantee: the paper's cross-tenant access that "will not compile."

## What Changes

- Introduce `memory` as a declared, governed resource with: `scope` (the access boundary, e.g. tenant), `retention` (lifetime), `retrieval` (policy), and `audit` (logging requirement).
- Bind each memory's `scope` to a **capability** (via `add-capability-effects`): every read/write requires the matching scope capability, granted only by entering that scope (e.g. `within customer`).
- Enforce the **native guarantee**: a read or write outside the declared scope is a **compile-time error** (`scope(...)` capability not granted), naming the memory and the missing scope.
- Provide retrieval operations: `nearest(query, k) within <scope>`, with **recency/exact retrieval only** in this change. **Decision:** the `retrieval semantic` half (same-space query vs stored embeddings) is **deferred and composed in `integrate-triage-flagship`** — it is not a requirement of this change.
- Enforce **retention and audit at runtime** (deterministic, in-memory store for v0.x): expired entries are not retrievable; audited accesses produce an audit record.

**Non-goals (deferred):** durable/external storage backends; ANN indexing/performance; hierarchical/nested scopes (single-level scope only); cross-scope sharing; value-level retrieval policy tuning. Honest boundary: **scope is statically checkable; retention and audit are runtime-enforced governance, not static guarantees** — and the compiler never verifies that retrieved context is *relevant* (§8).

## Capabilities

### New Capabilities
- `governed-memory`: the `memory` declaration (scope/retention/retrieval/audit), scope-bound read/write, scoped retrieval, and runtime retention/audit enforcement. Specialises `capability-effects` for the scope guarantee.

### Modified Capabilities
<!-- None as delta specs (bootstrap/capability-effects not yet archived). Consumes capability-effects conceptually; see Impact/Dependencies. -->

## Impact

- Depends on `bootstrap-language-core` (`type-system`, `host-runtime`, `language-grammar`) and on `add-capability-effects` (scope = capability).
- **No hard dependency on `add-embedding-primitive`:** this change ships recency/exact retrieval only, so it does not require embeddings. Semantic retrieval is composed later in `integrate-triage-flagship` (which hard-depends on `add-embedding-primitive`); only the flagship needs embeddings for retrieval.
- Required by `integrate-triage-flagship` (the triage `familiar` reads tenant-scoped `tickets`).
- Adds a runtime memory subsystem (store + retention clock + audit log), deterministic for tests.
- Build order: `bootstrap-language-core` must be implemented and archived before this change's specs can become delta-aware; this change adds new capabilities until then.
