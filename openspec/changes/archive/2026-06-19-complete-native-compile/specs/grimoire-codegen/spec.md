## ADDED Requirements

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

## MODIFIED Requirements

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

### Requirement: Compiled and interpreted execution are behaviourally equivalent
For any program accepted by `witch check`, running it via the interpreter (`witch run`) and via the compiled executable under the same seed SHALL produce identical observable output — and this SHALL hold for the whole language, including embeddings, governed memory, familiars, and the §6.3 flagship, not only the host subset and `divine`/`enact`. Equivalence SHALL be demonstrated on the Mock engine (byte-for-byte) and SHALL also hold per real engine: the compiled flagship's output SHALL match the interpreter's for the same engine binding. The interpreter is retained as the development loop; the compiled path is the ship path.

#### Scenario: Same program, same seed, same output across the whole language
- **WHEN** an example exercising embeddings, memory, and a familiar is run with `witch run --seed N` and as a `grimoire build` executable with seed N on the Mock engine
- **THEN** both produce identical stdout

#### Scenario: Flagship equivalence per engine
- **WHEN** the flagship is run interpreted and compiled against the same real engine binding
- **THEN** the two produce matching output for that engine, demonstrating compiled == interpreted across engines, not only under the Mock
