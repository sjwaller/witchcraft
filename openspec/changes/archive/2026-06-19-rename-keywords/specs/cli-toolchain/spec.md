## MODIFIED Requirements

### Requirement: check subcommand
The `witch` CLI SHALL provide a `check` subcommand that loads a `.witch` file, runs lexing, parsing, and type checking, reports any structural errors (including discharge and exhaustiveness errors), and exits 0 only if the program is well-typed. `check` SHALL NOT execute the program. Diagnostics SHALL refer to `define`, `speak`, and `listen` — not `fn` or `print`.

#### Scenario: check accepts define syntax
- **WHEN** a user runs `witch check ok.witch` on a program using `define` and `speak`
- **THEN** the tool exits 0 if well-typed

#### Scenario: check rejects fn
- **WHEN** a program still uses `fn`
- **THEN** `witch check` reports a syntax error

### Requirement: run subcommand
The `witch` CLI SHALL provide a `run` subcommand that type-checks and then executes a `.witch` file, writing `speak` output to stdout and reading `listen` from stdin, exiting 0 on success.

#### Scenario: run executes speak output
- **WHEN** a user runs `witch run hello.witch` where the file speaks `Greetings`
- **THEN** `Greetings` appears on stdout with a trailing newline
