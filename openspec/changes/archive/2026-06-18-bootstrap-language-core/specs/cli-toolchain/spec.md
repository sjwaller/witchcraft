## ADDED Requirements

### Requirement: witch check type-checks a source file
The `witch` CLI SHALL provide a `check` subcommand that loads a `.witch` file, runs lexing, parsing, and type checking, reports any structural errors (including discharge and exhaustiveness errors), and exits 0 only if the program is well-typed. `check` SHALL NOT execute the program.

#### Scenario: Well-typed program passes check
- **WHEN** a user runs `witch check ok.witch` on a structurally valid program
- **THEN** the CLI reports success and exits 0 without running the program

#### Scenario: Discharge error fails check
- **WHEN** a program uses an undischarged inferred value authoritatively
- **THEN** `witch check` reports the discharge error and exits non-zero

### Requirement: witch run executes a source file
The `witch` CLI SHALL provide a `run` subcommand that type-checks and then executes a `.witch` file, writing `print` output to stdout and exiting 0 on success. A program that fails type checking SHALL NOT be executed.

#### Scenario: Run a program
- **WHEN** a user runs `witch run hello.witch` where the file prints `Greetings`
- **THEN** the CLI prints `Greetings` to stdout and exits 0

#### Scenario: Type error blocks execution
- **WHEN** a user runs `witch run` on a program with a type error
- **THEN** the CLI reports the type error, does not execute the program, and exits non-zero

### Requirement: Decoder seed configuration
The CLI SHALL allow the decoder seed to be specified (e.g. via a flag or environment variable) so that `divine`-using programs execute deterministically and reproducibly.

#### Scenario: Same seed reproduces output
- **WHEN** a `divine`-using program is run twice with the same seed
- **THEN** both runs produce identical output

### Requirement: Human-readable diagnostics without panics
On any lexical, syntactic, type, or runtime error, the CLI SHALL print a human-readable diagnostic including the message and source position where available, and exit non-zero. It SHALL NOT panic or print a Rust backtrace for errors caused by user source. Success output SHALL NOT describe an inferred result as correct or verified (only as structurally well-formed).

#### Scenario: Syntax error reporting
- **WHEN** a `divine` block is missing a required clause
- **THEN** the CLI prints a positioned syntax-error diagnostic and exits non-zero

#### Scenario: Missing file
- **WHEN** a user runs `witch run` against a non-existent path
- **THEN** the CLI prints an error naming the missing path and exits non-zero
