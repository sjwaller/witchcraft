## ADDED Requirements

### Requirement: Human boundary I/O in compiled artifacts
The code generator SHALL lower `speak` statements and `listen(...)` calls to runtime ABI entry points that write to stdout and read one line from stdin respectively. Compiled executables produced by `grimoire build` SHALL behave identically to the interpreter for the same stdin/stdout interaction under the same seed for non-inference host code.

#### Scenario: Compiled speak matches interpreter
- **WHEN** a program containing only `speak "hi"` is built and run
- **THEN** stdout is `hi\n`, matching `witch run`

#### Scenario: Compiled listen matches interpreter
- **WHEN** a compiled program calls `listen("")` and stdin provides `action\n`
- **THEN** the returned glyph is `action`, matching `witch run`

## MODIFIED Requirements

### Requirement: Host control flow and functions compile
The code generator SHALL lower host control flow (`if`/`while`), function definitions (`define`), calls, arithmetic, comparisons, and `speak` to native code. Function names and diagnostics SHALL use `define`, not `fn`.

#### Scenario: define compiles to a callable native function
- **WHEN** `grimoire build` compiles a file defining `define double(x) { x + x }`
- **THEN** the artifact exports a callable that doubles its argument
