# human-io-boundary Specification

## Purpose
Define the human I/O surface of Witchcraft: stdout via `speak` and stdin via `listen`.

## Requirements
### Requirement: speak and listen are the human I/O surface
Witchcraft SHALL expose exactly two human-boundary I/O constructs at the language surface: `speak` for stdout output and `listen` for stdin input. Generic file or pipe writes SHALL NOT use `speak`. Generic file reads SHALL NOT use `listen`.

#### Scenario: speak is not aliased to print
- **WHEN** a program contains `print "x"`
- **THEN** parsing fails with a syntax error

#### Scenario: listen is a builtin, not a library import
- **WHEN** a program calls `listen("> ")`
- **THEN** type checking resolves it as the host builtin returning `glyph` without an oracle or import
