## Why

Witchcraft is a **first-class, standalone language**: people download it, install it, write `.witch` programs, and produce standalone executables — the same as any compiled language. It is explicitly **not** an embedded DSL, a Rust macro/library, or a "layer on top of Rust"; the Rust implementation is invisible to users (as C is to CPython users). This change delivers the "produce a standalone executable" half of that promise.

Witchcraft is intended to be a **compiled** language, not a parsed one. The v0.1 `bootstrap-language-core` shipped a tree-walking interpreter as a deliberate, documented bootstrap (its design names the "interpreter-first vs compiled-first" trade-off and bets the front-end is *not* throwaway): every `witch run` today re-lexes, re-parses, and re-walks the AST. That is fine as a dev loop but it is not how programs should ship. This change adds the code-generating backend that turns a type-checked `.witch` program into a distributable artifact that does **not** re-parse source at runtime and does **not** require Rust (or any toolchain) on the machine that runs it.

The critical constraint the paper imposes (§6.3, litmus test): compilation must **not** eliminate inference. The whole thesis is that the output type is a *generation-time* constraint — inference *is* the computation at the `divine` site. So the host language compiles ahead-of-time, but `divine`/`oracle`/`enact` retain a runtime component: codegen embeds the compiled grammar at each `divine` site and emits a call into the runtime decoder. A "compiled" Witchcraft is AOT host code plus an embedded, type-constrained inference runtime — closer to a compiled language with syscalls than to "everything resolved at build time."

## What Changes

- Introduce a **lowering IR** between the type checker and the backend (a small typed, stack-or-SSA intermediate form), so the front-end (`lexer`/`parser`/`typeck`/`grammar`) is shared and the backend is swappable.
- Add a **Cranelift native-AOT backend** (decided; see design D2) that emits a genuine **native executable** for the host language (`fn`, `let`/`var`, control flow, expressions, records/variants). Real native binaries + in-process inference I/O are the reasons; `grimoire build` bundles a linker (`lld`) so the build machine needs no external toolchain.
- Add the **`grimoire` build tool** (working surface `grimoire build app.witch -o app`, or `witch build`): type-check → lower → codegen → write a self-contained artifact.
- **Embed the type→grammar tables at each `divine` site** into the artifact, and emit a runtime call into the embedded `Decoder`; preserve discharge, `fallback`, exhaustive `enact`, confidence, and provenance semantics identically to the interpreter.
- Bundle the **runtime** (value model, decoder seam, mock decoder, provenance) into the artifact so it runs with **no Rust and no `.witch` source present**.
- Keep `witch run` as the **interpreter-backed dev loop** (like `go run`); `grimoire build` is the ship path. Both MUST produce identical observable behaviour for the same seed (a conformance requirement).

**Non-goals (deferred):** an optimizing compiler / performance work (correctness and self-containment first); the `memory`/`embedding`/`familiar` primitives themselves (each defines its own semantics in its own change; this change lowers only what exists at implementation time and later primitives add their own lowering); live model backends (still the deterministic decoder seam — real backends are v0.2+); the **Coven** package manager / registry (separate change); cross-compilation matrix and release packaging (that is `add-distribution`'s job — this change produces an artifact, distribution ships the toolchain that makes it).

## Capabilities

### New Capabilities
- `grimoire-codegen`: the lowering IR, the host-language code generator, the `grimoire build` command, the embedding of `divine` grammars into the artifact with a runtime decoder call, and the interpreter/compiled **behavioural-equivalence** guarantee.

### Modified Capabilities
<!-- None as delta specs yet: bootstrap is archived into openspec/specs/, but the codegen requirements are expressed as a new grimoire-codegen capability. It builds on type-system/host-runtime/constrained-decoder conceptually; see Impact. -->

## Impact

- Depends on `bootstrap-language-core` (`type-system`, `host-runtime`, `language-grammar`, `constrained-decoder`): codegen consumes the type-checked AST and reuses the existing type→grammar compiler.
- **Orthogonal to the four primitive changes** (`add-capability-effects`, `add-memory-primitive`, `add-embedding-primitive`, `add-familiar-primitive`): it is a new axis (how programs are produced), not a new primitive. It can be implemented independently after bootstrap. Each primitive change that lands after codegen MUST add its own lowering rule (a stated cross-change obligation).
- Pairs with `add-distribution`: codegen produces the artifact; distribution ships prebuilt `witch`/`grimoire` so users get the build/compile verbs without Rust.
- Establishes the IR seam the whole compiled toolchain inherits; coherence here matters disproportionately, like the bootstrap seams before it.
- Build order: `bootstrap-language-core` is implemented and archived; this change adds the `grimoire-codegen` capability on top of the baseline specs.
