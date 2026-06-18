## Context

`bootstrap-language-core` deliberately shipped a tree-walking interpreter and named the bet explicitly: the front-end (lexer, parser, type checker, type→grammar compiler) is shared with a future compiler and is therefore not throwaway. This change collects on that bet. The user requirement driving it: Witchcraft must be a compiled language whose users — once they have installed the toolchain — can write, check, run, build, and compile programs with **no Rust and no other toolchain**. Rust remains a maintainer/CI build-time dependency only.

The hard architectural fact that shapes every decision here: **inference cannot be compiled away.** The litmus test (§6.3) is that the output type is a generation-time constraint; deleting it changes what is generated. Therefore codegen lowers the *host* language ahead of time but leaves a runtime call at each `divine` site, with the compiled grammar embedded next to it. This is the same boundary a compiled language has at a syscall: the call site is compiled, the effect happens at runtime.

## Goals / Non-Goals

**Goals:**
- A typed lowering IR between `typeck` and the backend; front-end fully reused.
- A backend that emits a self-contained, runnable artifact for the host language.
- `grimoire build` produces an artifact that runs with no Rust and no `.witch` source.
- `divine`/`oracle`/`enact` semantics (grammar constraint, discharge, fallback, exhaustiveness, confidence, provenance) preserved bit-for-bit vs the interpreter under a fixed seed.
- `witch run` (interpreter) retained as the dev loop; compiled and interpreted runs are behaviourally equivalent.

**Non-Goals:**
- Optimization / performance (correctness + self-containment first).
- Defining memory/embedding/familiar semantics (they add their own lowering).
- Real model backends (decoder seam stays; v0.2+).
- Coven package manager; the cross-platform release matrix (that is `add-distribution`).

## Decisions

### D1: A small typed lowering IR sits between typeck and the backend
The type checker emits a typed IR (lean: a simple stack-based bytecode with explicit value tags, or minimal SSA) rather than codegen reading the AST directly. **Why:** keeps the backend swappable (interpreter today, Cranelift native tomorrow, WASM later if sandboxing is needed), gives a stable target for each later primitive's lowering, and isolates "what the program means" from "how this target encodes it." *Alternative:* AST-directed codegen — rejected, it couples every backend to surface syntax and forces each primitive to re-handle every target.

### D2: Backend target — Cranelift native AOT (DECIDED)
The backend is **Cranelift**, producing genuine **native executables**. The IR (D1) keeps this reversible, but it is the chosen target, not a lean.

The candidates considered:

| Target | Pros | Cons |
|--------|------|------|
| **Cranelift** (native code, Rust-native codegen) — **CHOSEN** | real native binaries; fast compile times; no external/LLVM build dependency; the existing Rust runtime (decoder, value model, oracle adapters) links straight in, so **inference I/O is in-process** (FFI to llama.cpp, sockets to Ollama, API calls) with no sandbox boundary | we own the calling convention + value/memory model; producing the final executable needs a **linker** (we bundle one — see below) |
| **WASM** (embed wasmtime; or standalone `.wasm`) | portable, sandboxed, clean host-import boundary | all inference I/O is pushed across the sandbox to the native host anyway; not a bare native binary without an embedder; extra indirection for the trusted local-model case |
| **LLVM** (via inkwell) | best optimization, true native | heavy build dependency, slow builds — and optimization is **wasted on an inference-bound language**; contradicts "small, verifiable" |
| **Portable bytecode + bundled VM** | smallest lift; no linker | weakest "really compiled?" optics; host-code perf below native |

**Why Cranelift:** (1) The defining workload of an AI-native language is **inference**, which Cranelift handles best by letting the runtime call models **in-process** — native function calls / FFI, no host-import trampoline. (2) It produces **real native executables**, satisfying "download, install, create executables" literally. (3) It is **Rust-native** — no external compiler/LLVM on the maintainer build machine, consistent with "small, verifiable changes." (4) Host-code optimization (LLVM's edge) is irrelevant here because the cost is dominated by the `divine` call, so Cranelift's lighter optimization is no loss.

**Costs accepted with this decision:**
- **We own the host value/memory model** (records, variants, provenance-bearing values) — see Open Questions.
- **A linker is required** to emit the executable. To honour "no toolchain on the build machine," `grimoire build` **bundles a linker (`lld`)** rather than shelling out to a system `cc`/`ld`; system-linker use is a fallback, never a requirement.

**Reopening trigger (the one thing that would revisit this):** if **running untrusted `.witch` programs in a sandbox** (multi-tenant, plugins, edge) becomes a product goal, add **WASM as a second backend / distribution target** behind the same IR — it does not replace Cranelift, it sits alongside it.

### D3: `divine` stays a runtime call with an embedded grammar (non-negotiable)
At each `divine` site, codegen serialises the compiled type→grammar table into the artifact and emits a call into the bundled runtime decoder, followed by the discharge gate and `fallback` branch as ordinary compiled control flow; `enact` lowers to a compiled exhaustive dispatch over the variant tag. **Why:** preserves the litmus property in compiled form — the grammar is *in the artifact*, so it still shapes generation; remove the type and the embedded grammar changes. *Rejected:* pre-computing inference at build time (destroys the thesis) or shipping the raw type for runtime re-derivation (wasteful, and re-introduces a parser).

### D4: Artifact shape — self-contained by default
`grimoire build` produces a **single self-contained native executable** (Cranelift codegen + the runtime — value model, decoder seam, mock decoder, provenance — statically linked in via the bundled `lld`), so deployment is one file with no Rust and no source. **Why:** matches the user requirement most literally. A split "module + shared runtime lib" mode is a possible later option, not the default.

### D5: Oracle backends are linked behind the seam, selected at build/deploy — not a language dependency
The decoder/oracle seam compiles in; *which* backend answers is chosen by configuration: the deterministic **mock decoder is bundled** (so an artifact always runs offline and reproducibly); real backends (Ollama/llama.cpp/API, v0.2+) attach behind the same seam as **natively linked adapters** — in-process FFI to llama.cpp, sockets to Ollama, or API calls, with no sandbox boundary (a direct benefit of the Cranelift native target, D2). **Why:** keeps "install Witchcraft, no Rust" true; choosing an inference backend is a deployment choice (like Postgres vs SQLite), not a toolchain dependency. Provenance records which backend + seed produced each value.

### D6: `witch run` is the dev loop; `grimoire build` is the ship path; they must agree
The interpreter is **retained**, not deleted — it is the fast-iteration path (`go run` analogue). A **conformance requirement** locks that the compiled artifact and the interpreter produce identical observable output for the same program and seed. **Why:** keeps the dev loop snappy while making the compiled path authoritative for shipping, and the equivalence test prevents the two backends from drifting.

### D7: No Rust at the user boundary (shared with `add-distribution`)
This change makes a Rust-free *run* of compiled artifacts true; `add-distribution` makes a Rust-free *install of the toolchain itself* true (prebuilt `witch`/`grimoire`). Together they satisfy "install once → write/run/build/compile, no Rust." Stated here so the boundary between the two changes is explicit.

### D8: Host value memory model — reference counting (DECIDED)
The compiled runtime represents a host value as a small **tagged union**: scalars (`spark`, `bool`, unit) are **unboxed**; heap payloads (`glyph` text, `record`/`variant` field arrays, and the boxed inner value + provenance of `Inferred<T>`) are **reference-counted**. When a value's count reaches zero its payload is freed and its children are decremented.

**Why reference counting (not arena, not tracing GC):**
- **Witchcraft host values are immutable and acyclic** — there is no construct that creates a mutable reference or a cycle (records/variants are built bottom-up from existing values; nothing can point back). Acyclicity is exactly the precondition under which reference counting is **complete**: no cycle collector is ever needed.
- **It reclaims loop-local memory.** A bump **arena per run** was considered and rejected as the default: a bounded program with a large `while` loop (e.g. allocating a `glyph` per iteration) would grow the arena unboundedly. Refcounting frees each iteration's values as they fall out of scope.
- **A tracing GC is unjustified** — it exists to reclaim cycles, which cannot occur here; it would add a runtime and pauses for no benefit.

**Kept reversible:** allocation/retain/release go through a **narrow runtime interface**, so a later **region/arena fast-path for hot bounded scopes** (e.g. a single `divine`/`familiar` pass) can be added as an optimization without changing codegen or the value representation. The interpreter and the compiled runtime SHALL share the same logical value semantics so the D6 equivalence holds.

**Named reopening trigger:** introducing **mutable references or cyclic values** into the language (not currently possible) would break acyclicity and force a cycle-aware collector. Nothing else reopens this.

## Risks / Trade-offs

- **Owning the native value/memory model is non-trivial** → resolved by D8 (reference counting over immutable acyclic values); start with the smallest existing host-language subset, keep the IR value model explicit and small, and lean on the interpreter as the equivalence reference. Refcount inc/dec is noise next to inference cost.
- **Bundled linker (`lld`) adds maintenance + binary size** → accepted to keep the build machine toolchain-free; system-linker fallback exists if a platform needs it.
- **Compiled vs interpreted semantic drift** → mitigated by the D6 equivalence conformance suite run in CI on every example.
- **Embedding grammars bloats artifacts** → grammars for v0.1 types are tiny (finite variants, integer ranges, bounded text); revisit only if richer types arrive.
- **"Compiled" overclaim** → be honest in docs: the host language is AOT; inference is a runtime, type-constrained effect. A green compile is still structural, not semantic (§8).

## Open Questions

- **Bundled linker mechanics:** ship `lld` via the `lld`-as-library route vs a vendored binary; what is the per-platform integration cost? (Decision is *that* we bundle, D2; *how* is open.)
- **Oracle adapter ABI (native):** the minimal in-process interface for a real backend behind the decoder seam (grammar in → value + confidence + provenance out), so v0.2 backends attach without codegen changes.
- **Artifact entry/CLI:** does a compiled executable accept `--seed` and program args like `witch run` (argv vs env)?
- **How later primitives lower:** capability checks are compile-time (erase at runtime), but memory (runtime store), embedding (vector ops), and familiar (bounded loop) each need a Cranelift lowering rule — note the obligation now; specify per-change.
