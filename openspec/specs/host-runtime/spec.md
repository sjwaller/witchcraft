# host-runtime Specification

## Purpose
TBD - created by archiving change bootstrap-language-core. Update Purpose after archive.
## Requirements
### Requirement: Host value model
The interpreter SHALL evaluate the host language using a value model covering at minimum: `spark` (numeric), boolean, `glyph` (text), record values, variant values, `oracle` values, inferred values (see type-system), and a unit value. Every value SHALL report its type for use in diagnostics.

#### Scenario: Evaluate primitive literals
- **WHEN** the program evaluates `42`, `true`, and `"hex"`
- **THEN** they produce a `spark`, a boolean, and a `glyph` value respectively

#### Scenario: Construct and read a record
- **WHEN** a record value is constructed and a field is read
- **THEN** the field access yields the value assigned to that field

### Requirement: Bindings and mutation
`let` SHALL introduce an immutable binding; reassigning it SHALL be an error. `var` SHALL introduce a mutable binding that MAY be reassigned. Reading an undefined name SHALL raise a clear error naming the identifier.

#### Scenario: let is immutable
- **WHEN** a program declares `let x = 1` and later attempts `x = 2`
- **THEN** the program reports an error stating a `let` binding cannot be reassigned

#### Scenario: var is mutable
- **WHEN** a program declares `var counter = 0` and later executes `counter = counter + 1`
- **THEN** the value of `counter` becomes `1`

### Requirement: Lexical scoping
Names declared inside a `define`, `while`, or `if` block SHALL be scoped to that block and its descendants and SHALL NOT leak into the enclosing scope. Inner scopes SHALL be able to read enclosing names and mutate enclosing `var`s.

#### Scenario: Inner declaration does not leak
- **WHEN** a name is declared with `let` inside a `while` body
- **THEN** referencing it after the loop raises an undefined-identifier error

#### Scenario: Inner scope mutates outer var
- **WHEN** a `while` body assigns to a `var` declared in the enclosing scope
- **THEN** the enclosing `var` reflects the mutation after the loop

### Requirement: Functions and control flow
A `define` SHALL be callable with positional arguments, execute in a fresh scope, and return its result value (or unit). The interpreter SHALL evaluate `if`/`else`, `while` (re-checking the condition each iteration), arithmetic (`+ - * /`), comparison (`< <= > >= == !=`), and boolean (`and or not`, short-circuiting) operators, and `speak` (textual rendering + newline to stdout). Division by zero SHALL raise a clean error, not a panic.

#### Scenario: Function returns a value
- **WHEN** `define add(a, b) { a + b }` is called as `add(2, 3)`
- **THEN** the call evaluates to `5`

#### Scenario: while iterates until false
- **WHEN** `var n = 0` and `while n < 3 { speak n; n = n + 1 }` runs
- **THEN** the program speaks `0`, `1`, `2` and stops

#### Scenario: Division by zero is a clean error
- **WHEN** a program evaluates `1 / 0`
- **THEN** the runtime raises a division-by-zero error rather than panicking

### Requirement: Human boundary I/O
The interpreter SHALL provide `listen(prompt: glyph) -> glyph`, which performs a blocking read of one line from standard input, strips a trailing newline if present, and returns the line as a glyph. The `prompt` argument is available to the program for composition; the runtime SHALL NOT perform implicit file or network I/O. `speak` SHALL write only to standard output.

#### Scenario: listen reads a line
- **WHEN** stdin contains `open the door\n` and the program evaluates `listen("")`
- **THEN** the result is the glyph `open the door`

#### Scenario: speak writes to stdout
- **WHEN** the program executes `speak "hello"`
- **THEN** `hello` followed by a newline is written to stdout

### Requirement: Evaluate record literals
The interpreter SHALL evaluate a record literal by evaluating each field expression and constructing a record value whose fields are those results.

#### Scenario: Construct a record value
- **WHEN** `{ a: 1, b: "x" }` is evaluated
- **THEN** the result is a record value with fields `a = 1` and `b = "x"`

