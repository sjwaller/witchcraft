## Context

`add-grimoire-codegen` built the AOT path for the host language plus `divine`/`enact` and made compiled/interpreted equivalence a hard requirement. The three later primitives were then implemented interpreter-only, with lowering deliberately rejecting them so the workspace stayed green. The exact rejections in `crates/witchcraft/src/lower.rs` today are:

```
lower_expr:  Expr::List          → "list literals are not supported by the compiler yet"
             Expr::Method(embed)  → "method `embed` is not supported by the compiler yet"
             (similarity/nearest)  → lowered as Call to an undefined function (no ABI builtin)
lower_stmt:  Stmt::MemoryDecl     → "governed memory is not supported by the compiler yet"
             Stmt::Within         → "`within` is not supported by the compiler yet"
lower(items): Item::Familiar      → "familiars are not supported by the compiler yet"
```

Each rejection is a TODO with a known shape. This change turns each into a lowering rule plus the minimal runtime/ABI it requires, then proves equivalence. Nothing about the *language* changes — these constructs already parse, type-check, and run interpreted.

## Goals / Non-Goals

**Goals:** embedding, memory, and familiar lower to native code; the flagship builds and matches the interpreter byte-for-byte under one seed (`Mock` engine); the equivalence discipline now covers the whole language; the **falsification/litmus test and the cross-engine (local + frontier) flagship swap also hold for the compiled binary**, not just the interpreter.
**Non-Goals:** authoring engines (the `add-inference-runtime` contract; the compiled path only *calls* it); durable memory; semantic retrieval; any new surface syntax.

## What each construct needs to lower

```
                 interpreter today            compiled (this change)
 embedding  ───  Value::Embedding/List   →    runtime Value::Embedding + Value::List (RC heap)
 embed      ───  hash(text, space)       →    w_embed(intent_id,input) → Engine::embed (mock=hash)
 similarity ───  cosine                  →    w_similarity (shared cosine + tie-break routine)
 nearest    ───  sort desc, stable       →    w_nearest (same routine; returns a list value)
 list [..]  ───  Value::List             →    list builder ABI + iteration
 memory     ───  HashMap store + clock   →    global runtime registry: store, logical clock, audit
   .write   ───  push (tick,val)         →    w_mem_write
   .recent  ───  filter+sort newest      →    w_mem_recent → list value
   within   ───  exec body (erased)      →    lower body; scope is compile-time only
   advance  ───  clock += n              →    w_advance ;  audit_log → w_audit_log
 familiar   ───  call like a fn          →    lower_function (permits/bounded are compile-time)
```

## Decisions

### D1: Familiar lowers as a function (smallest change first)
A familiar is already callable like a function; its permits and single-pass bound are compile-time properties the checker enforces. So lowering removes the `Item::Familiar` rejection and routes it through the existing `lower_function`. Calls already dispatch through the function path. **Why:** no runtime concept is needed — the familiar's novelty is entirely in the checker.

### D2: Memory is a global runtime registry mirroring the interpreter
The compiled runtime gains a process-global (thread-local) memory registry with the same logical clock, retention filter, and audit log as `crates/witchcraft/src/interp.rs`. `MemoryDecl` lowers to `w_mem_register(name, retention, audit)`; `within` lowers to its body (scope erased); `mem.write`/`mem.recent` and the `advance`/`audit_log` affordances lower to ABI calls. **Why:** matches the interpreter's model exactly, which is what equivalence requires; durability is out of scope and slots behind the same registry later.

### D3: List and Embedding are reference-counted heap values
`Value::List` and `Value::Embedding { space, vector, provenance }` join glyph/record/variant as RC heap payloads in the 16-byte `repr(C)` value, reusing the existing retain/release discipline (immutable, acyclic — no cycle collector). **Why:** consistent with the runtime's existing memory model; `nearest` returning a list and `embed` returning a vector both need owned heap values.

### D4: One shared arithmetic + tie-break routine for `similarity`/`nearest`
The cosine computation and `nearest`'s descending-sort-with-stable-index tie-break are factored into a single routine shared by the interpreter and the compiled runtime (or duplicated with a golden test pinning equality). **Why:** float non-associativity could otherwise make compiled and interpreted output diverge; equivalence demands bit-identical results under the `Mock` engine.

### D5: Compiled `embed`/`divine` route through the inference-runtime contract
`w_embed` calls `Engine::embed` and the compiled `divine` calls `witch_ai_infer` (`Engine::infer`) from `add-inference-runtime`; the `Mock` engine's embed/infer are today's deterministic hash/PRNG, so equivalence tests stay reproducible and offline. Real engines (llama.cpp local, frontier network) are opt-in purely via the manifest — **the compiled binary selects its engine at load exactly as the interpreter does**, so the same artifact runs local or network with no rebuild. The codegen also stops dropping the `from (...)` inputs (Break 3's residue): the prompt is threaded into `witch_ai_infer`. **Why:** this is the hard dependency on change A — compiled AI primitives need the engine contract to exist; it is also what makes the compiled litmus/engine-swap proofs (below) possible.

### D6: Equivalence — and the compiled litmus + engine-swap — are the acceptance bar
Three nested proofs, strongest last:
1. **`Mock` equivalence (byte-for-byte).** Extend `assert_compiled_equals_interpreted` to embedding/memory/familiar programs and `examples/triage_flagship.witch`: `grimoire build` output equals `witch run` under the same seed (`Mock` engine).
2. **Compiled litmus.** The falsification test of `add-inference-runtime` (D8 there) runs against the **native binary** with the real engine(s): real grammar ⇒ in-grammar by construction; weakened grammar ⇒ genuinely different; indistinguishable ⇒ fail loudly. Proves the compiler did not silently degrade constrained decoding to validate-after.
3. **Compiled engine-swap.** A `grimoire build` flagship binary runs against a real **local** model and the **frontier** API selected purely by manifest, no source change, and its output equals the interpreter's per engine. This is "compiled == interpreted" *across engines*, the real bar for a compiled AI-first language.

**Why:** (1) is the cheapest reuse of the existing discipline; (2) and (3) are what stop the compiled path from quietly being a wrapper while the interpreter is honest.

## Risks / Trade-offs

- **Float/sort drift between paths (D4)** → single shared routine + golden equality test.
- **Memory global state in compiled artifacts (D2)** → thread-local, reset per run like the seed/sink; documented as in-memory v0.x, behind the same interface for later durability.
- **Binary size from new ABI surface** → small; pure-Rust, no model code (the heavy engine stays optional via change A — `Mock` is the default, real engines link only when selected).
- **Coupling to change A** → the AI calls (`w_embed`, `witch_ai_infer`) need change A's contract; memory/familiar are independent, so the change can be staged (familiar+memory first, embed/divine native after A lands) if needed.
- **Compiled litmus could fail where interpreted passes** → that would be a *true* finding (the compiler degraded constrained decoding); D6.2 makes it loud rather than hidden. Mitigation: both paths funnel through the *same* `Engine` contract, so there is one place enforcement can live.

## Dependency graph (against archived changes)

```
bootstrap-language-core ─┬─ add-grimoire-codegen ──────────────┐
                         │     (IR, Cranelift, RC runtime, ABI) │
                         ├─ add-capability-effects ──── permit(network) ─┐
                         ├─ add-embedding-primitive ─┐                   │
                         ├─ add-memory-primitive ────┤ (constructs)      │
                         ├─ add-familiar-primitive ──┤                   │
                         ├─ integrate-triage-flagship┘ (equivalence tgt) │
                         └─ add-distribution ── offline-mock-default ────┤
                                                                         │
   NEW:  add-inference-runtime  ◄───────────────────────────────────────┘
                 │  (NEED/POLICY/BINDING/ENGINE contract, witch_ai_infer ABI,
                 │   manifest + load-time resolution, llama.cpp + frontier
                 │   engines, Mock demoted, Engine::embed seam, falsification test)
                 ▼
         complete-native-compile
              (lower embedding/memory/familiar to native; route divine/embed
               through the Engine contract; compiled == interpreted for the
               whole language + flagship; compiled litmus + engine-swap proofs)
```

## Open Questions (with leans)

- **Memory store identity/lifetime in the artifact** → **global thread-local registry**, reset per run (mirrors interpreter + seed/sink).
- **List/Embedding value representation** → **RC heap payloads** in the 16-byte value, like glyph/record.
- **Real engine vs mock for compiled `embed`/`divine`** → **`Mock` engine for equivalence**; real engines opt-in via change A's manifest, selected at load by the compiled binary just like the interpreter.
- **Float/tie-break determinism** → **one shared cosine + stable-sort routine** so both paths are bit-identical under the `Mock` engine.
