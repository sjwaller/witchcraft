## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Human boundary I/O
The interpreter SHALL provide `listen(prompt: glyph) -> glyph`, which performs a blocking read of one line from standard input, strips a trailing newline if present, and returns the line as a glyph. The `prompt` argument is available to the program for composition; the runtime SHALL NOT perform implicit file or network I/O. `speak` SHALL write only to standard output.

#### Scenario: listen reads a line
- **WHEN** stdin contains `open the door\n` and the program evaluates `listen("")`
- **THEN** the result is the glyph `open the door`

#### Scenario: speak writes to stdout
- **WHEN** the program executes `speak "hello"`
- **THEN** `hello` followed by a newline is written to stdout
