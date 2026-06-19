# grimoire-codegen Specification

## Purpose
Define the ahead-of-time compilation path for Witchcraft: a typed lowering IR
between the type checker and the backend, a Cranelift backend that emits native
code, a reference-counted compiled runtime, and `grimoire build`, which links them
into a self-contained native executable. The compiled (ship) path and the
interpreter (dev loop) must produce identical observable output, and inference
must remain a runtime, type-constrained effect — never pre-computed at build time.

## Requirements
### Requirement: Lowering IR between type checker and backend
The toolchain SHALL lower a type-checked program into an explicit intermediate representation (IR) before code generation, rather than generating code directly from the AST. The IR SHALL be the single target that the code generator and any future backend consume, and the unit that later language primitives add lowering rules to.

#### Scenario: A checked program lowers to IR
- **WHEN** a well-typed program is compiled
- **THEN** the toolchain produces an IR for the whole program before any backend runs

#### Scenario: Ill-typed programs are never lowered
- **WHEN** a program fails type checking
- **THEN** no IR is produced and no artifact is generated

### Requirement: grimoire build produces a self-contained native executable
The toolchain SHALL provide a build command (`grimoire build <file.witch> -o <out>`) that type-checks, lowers, and generates a native executable via the Cranelift backend. The executable SHALL be self-contained: it SHALL run with no Rust toolchain, no `cargo`, and no `.witch` source present, with the runtime (value model, decoder seam, mock decoder, provenance) linked in. The executable SHALL accept `--seed <n>`, matching `witch run`.

#### Scenario: Build then run without source or toolchain
- **WHEN** a user runs `grimoire build app.witch -o app` and then runs `./app` on a machine with no Rust and with the source removed
- **THEN** the program executes and produces its output

#### Scenario: Build refuses an ill-typed program
- **WHEN** a user runs `grimoire build` on a program with a type error
- **THEN** the build reports the error, produces no executable, and exits non-zero

### Requirement: divine compiles to an embedded grammar plus a runtime decode call
For each `divine` site, the code generator SHALL embed the compiled output-type→grammar into the artifact and emit a runtime call into the bound engine via the inference-runtime ABI (`witch_ai_infer`), passing the evaluated input, followed by the confidence discharge and `fallback` branch as compiled control flow. Inference SHALL NOT be resolved at build time. The grammar SHALL constrain generation during decoding, never validate-after; a litmus-strict site SHALL refuse a non-litmus-safe engine at load. `enact` SHALL compile to an exhaustive dispatch over the variant tag.

#### Scenario: Grammar travels in the artifact and constrains generation
- **WHEN** a program containing a `divine` of a refined/variant output type is built and run on a litmus-safe engine
- **THEN** the generated value satisfies the output type's grammar by construction (a refined number stays in range; only declared variants occur), produced by the bound engine at runtime

#### Scenario: Compiled litmus asserts masking occurred
- **WHEN** the same program is built with the output type present versus structurally weakened and run on a real litmus-safe engine under the same seed
- **THEN** the compiled run demonstrates that the real grammar forbade a token the weakened grammar permitted during generation (not merely that the final outputs differ)

#### Scenario: Inputs reach the engine in compiled form
- **WHEN** a compiled `divine` site with `from (...)` inputs executes
- **THEN** the resolved inputs are passed to the engine via `witch_ai_infer` rather than evaluated for effect and discarded

### Requirement: Heap host values use reference counting
The compiled runtime SHALL represent host values with unboxed scalars and reference-counted heap payloads (`glyph` text, record/variant fields, and the inner value/provenance of an inferred value), freeing a payload when its count reaches zero and decrementing its children. Because host values are immutable and acyclic, the runtime SHALL NOT require a cycle collector.

#### Scenario: Loop-local values are reclaimed
- **WHEN** a bounded program allocates heap values inside a loop that iterates many times
- **THEN** memory for values that have gone out of scope is reclaimed during execution rather than retained until program exit

### Requirement: Compiled and interpreted execution are behaviourally equivalent
For any program accepted by `witch check`, running it via the interpreter (`witch run`) and via the compiled executable under the same seed SHALL produce identical observable output — and this SHALL hold for the whole language, including embeddings, governed memory, familiars, and the §6.3 flagship, not only the host subset and `divine`/`enact`. Equivalence SHALL be demonstrated on the Mock engine (byte-for-byte) and SHALL also hold per real engine: the compiled flagship's output SHALL match the interpreter's for the same engine binding. The interpreter is retained as the development loop; the compiled path is the ship path.

#### Scenario: Same program, same seed, same output across the whole language
- **WHEN** an example exercising embeddings, memory, and a familiar is run with `witch run --seed N` and as a `grimoire build` executable with seed N on the Mock engine
- **THEN** both produce identical stdout

#### Scenario: Flagship equivalence per engine
- **WHEN** the flagship is run interpreted and compiled against the same real engine binding
- **THEN** the two produce matching output for that engine, demonstrating compiled == interpreted across engines, not only under the Mock

### Requirement: Bundled runtime; linker is a configurable seam
`grimoire build` SHALL link the runtime into the artifact without requiring a separately installed Rust toolchain or `cargo`: the runtime SHALL be carried by the `grimoire` binary itself. The linker that emits the final executable SHALL be a configurable seam (selectable compiler driver and `-fuse-ld` flavour). Fully removing the dependency on a system linker — bundling a linker (e.g. `lld`) together with the per-platform SDK handling that implies — is owned by the distribution-packaging capability. A green build SHALL remain a structural guarantee only, never an assertion that inferred values are correct.

#### Scenario: Build without a Rust toolchain or cargo
- **WHEN** `grimoire build` runs on a machine that has the `grimoire` binary but no Rust toolchain and no `cargo`
- **THEN** it links the embedded runtime and produces a working executable

#### Scenario: Linker is selectable
- **WHEN** a user configures the compiler driver or linker flavour for `grimoire build`
- **THEN** the configured driver/linker is used to emit the executable

### Requirement: Embeddings and lists lower to native code
The code generator SHALL lower list literals, `oracle.embed`, and the `similarity`/`nearest` builtins to native code, replacing the current lowering rejections. The compiled runtime SHALL represent `Value::List` and `Value::Embedding { space, vector, provenance }` as reference-counted heap values, and SHALL provide ABI entry points to build/iterate lists, embed an input (`w_embed`, routed through the inference-runtime `Engine::embed`; the Mock engine's deterministic hash is the test path), and compute `w_similarity`/`w_nearest`. The same-space restriction enforced at compile time SHALL be mirrored by the runtime arithmetic.

#### Scenario: Embedding program builds and runs natively
- **WHEN** a program using `oracle.embed`, `similarity`, `nearest`, and list literals is built with `grimoire build`
- **THEN** it produces a working native executable whose output matches the interpreter under the same seed on the Mock engine

#### Scenario: similarity/nearest are bit-identical across paths
- **WHEN** `nearest`/`similarity` run compiled and interpreted on the Mock engine with the same inputs and seed
- **THEN** both produce identical results, including ordering and tie-breaks (one shared arithmetic + stable-sort routine)

### Requirement: Governed memory lowers to native code
The code generator SHALL lower `memory` declarations, `within` blocks, and the `mem.write`/`mem.recent`/`advance`/`audit_log` operations to native code. The compiled runtime SHALL provide a memory registry mirroring the interpreter — per-store entries, a logical clock, retention filtering, and an audit log — behind ABI entry points (`w_mem_register`, `w_mem_write`, `w_mem_recent`, `w_advance`, `w_audit_log`). Scope is a compile-time capability and SHALL be erased before lowering; `within` SHALL lower to its body.

#### Scenario: Memory program builds and runs natively
- **WHEN** a program declaring `memory`, writing within a scope, and reading recent entries is built with `grimoire build`
- **THEN** it produces a working native executable whose output (including retention expiry and audit log) matches the interpreter under the same seed

#### Scenario: Scope is erased, not enforced at runtime
- **WHEN** a `within <scope>` block is lowered
- **THEN** the scope capability has already been checked at compile time and leaves no runtime check; the body lowers as an ordinary block

### Requirement: Familiars lower to native code
The code generator SHALL lower a `familiar` item as an ordinary function, removing the current rejection. Permits and the single-pass/bounded property SHALL remain compile-time checks with no runtime representation; a familiar call SHALL dispatch through the existing function-call path.

#### Scenario: Familiar program builds and runs natively
- **WHEN** a program declaring and calling a `familiar` is built with `grimoire build`
- **THEN** it produces a working native executable whose output matches the interpreter under the same seed, and the familiar's permits remain a compile-time-only guarantee

### Requirement: The compiled binary selects its engine by manifest with no source change
A native executable produced by `grimoire build` SHALL resolve its inference needs against the deployment manifest at load, exactly as the interpreter does, selecting a local or network engine without any change to the program source or any rebuild of source. The same built binary SHALL run against a real local model and against the network engine purely by changing the manifest. The compiled `divine` SHALL call the inference-runtime engine ABI (`witch_ai_infer`) with the evaluated input threaded through; it SHALL NOT drop the `from (...)` inputs.

#### Scenario: One binary, two engines, no source change
- **WHEN** a `grimoire build` flagship binary is run with a manifest binding its needs to a local llama model, then run again with a manifest binding them to the frontier API
- **THEN** both runs use the same binary and the same source, selecting the engine purely by manifest

#### Scenario: Compiled inference acts on the input
- **WHEN** a compiled `divine` site executes
- **THEN** the evaluated input is passed to the bound engine via `witch_ai_infer` rather than discarded
