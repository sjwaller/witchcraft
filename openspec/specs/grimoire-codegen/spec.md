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
For each `divine` site, the code generator SHALL embed the compiled output-type→grammar into the artifact and emit a runtime call into the linked decoder, followed by the confidence discharge and `fallback` branch as compiled control flow. Inference SHALL NOT be resolved at build time. `enact` SHALL compile to an exhaustive dispatch over the variant tag, threading provenance into the enacted action.

#### Scenario: Grammar travels in the artifact
- **WHEN** a program containing a `divine` of a refined/variant output type is built and run
- **THEN** the generated value satisfies the output type's grammar (e.g. a refined number stays in range; only declared variants occur), produced by the embedded decoder at runtime

#### Scenario: Litmus holds in compiled form
- **WHEN** the same program is built with the output type present versus structurally removed, under the same seed
- **THEN** the runtime-generated output differs

#### Scenario: Low-confidence inference cannot reach enact
- **WHEN** a built program's discharge sees a confidence below its threshold
- **THEN** the `fallback` branch runs and the inferred value never flows into `enact` or downstream use

### Requirement: Heap host values use reference counting
The compiled runtime SHALL represent host values with unboxed scalars and reference-counted heap payloads (`glyph` text, record/variant fields, and the inner value/provenance of an inferred value), freeing a payload when its count reaches zero and decrementing its children. Because host values are immutable and acyclic, the runtime SHALL NOT require a cycle collector.

#### Scenario: Loop-local values are reclaimed
- **WHEN** a bounded program allocates heap values inside a loop that iterates many times
- **THEN** memory for values that have gone out of scope is reclaimed during execution rather than retained until program exit

### Requirement: Compiled and interpreted execution are behaviourally equivalent
For any program accepted by `witch check`, running it via the interpreter (`witch run`) and via the compiled executable under the same seed SHALL produce identical observable output. The interpreter is retained as the development loop; the compiled path is the ship path.

#### Scenario: Same program, same seed, same output
- **WHEN** an example is run with `witch run --seed N` and as a `grimoire build` executable with seed N
- **THEN** both produce identical stdout

### Requirement: Bundled runtime; linker is a configurable seam
`grimoire build` SHALL link the runtime into the artifact without requiring a separately installed Rust toolchain or `cargo`: the runtime SHALL be carried by the `grimoire` binary itself. The linker that emits the final executable SHALL be a configurable seam (selectable compiler driver and `-fuse-ld` flavour). Fully removing the dependency on a system linker — bundling a linker (e.g. `lld`) together with the per-platform SDK handling that implies — is owned by the distribution-packaging capability. A green build SHALL remain a structural guarantee only, never an assertion that inferred values are correct.

#### Scenario: Build without a Rust toolchain or cargo
- **WHEN** `grimoire build` runs on a machine that has the `grimoire` binary but no Rust toolchain and no `cargo`
- **THEN** it links the embedded runtime and produces a working executable

#### Scenario: Linker is selectable
- **WHEN** a user configures the compiler driver or linker flavour for `grimoire build`
- **THEN** the configured driver/linker is used to emit the executable
