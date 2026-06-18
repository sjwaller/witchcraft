## 1. Memory declaration + scope-as-capability

- [x] 1.1 `memory <name> { scope S, retention N unit, retrieval ..., audit required|optional }` declaration (settings are contextual; `memory`/`within` are keywords)
- [x] 1.2 Checker records the memory (name → scope); a memory without a `scope` is a compile error
- [x] 1.3 `within <scope> { ... }` grants `scope(<scope>)` to its body (consumes capability-effects); does not leak past the region

## 2. Governed access + retrieval

- [x] 2.1 `mem.write(v)` and `mem.recent(k)` require the memory's `scope(S)` capability; out-of-scope access is a compile error naming the memory and scope (the cross-tenant leak that will not compile)
- [x] 2.2 Recency/exact retrieval only in this change (semantic retrieval is composed in the flagship per design D4); `nearest` signature kept stable <!-- mem.recent(k); mem.nearest aliases recency for now, semantic in flagship -->
- [x] 2.3 Runtime: deterministic in-memory store with a logical clock; retention filters expired entries; `audit required` appends an audit record per governed access (`advance`/`audit_log` affordances)

## 3. Scope, erasure, tests

- [x] 3.1 Memory is interpreter-only in v0.x; lowering rejects `memory`/`within` with a clear diagnostic
- [x] 3.2 Tests: declare; missing-scope rejected; in-scope ok; grant does not leak; out-of-scope (cross-tenant) compile error; recency retrieval; expired entries excluded; audited access records; structural-only wording <!-- crates/witchcraft/tests/memory.rs -->
- [x] 3.3 `openspec validate add-memory-primitive --strict`; README note
