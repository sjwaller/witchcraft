## MODIFIED Requirements

### Requirement: Un-themed host syntax
The lexer and parser SHALL provide plain, conventional keywords for mundane constructs and SHALL NOT theme them: `define` (function), `let` (immutable binding), `var` (mutable binding), `while`, `if`/`else`, and `return`. Occult vocabulary SHALL be reserved for constructs that name something genuinely new to the language or the human boundary (`oracle`, `summon`, `divine`, `enact`, `fallback`, `speak`, `listen`). Tokens SHALL carry source position (line and column). The keywords `fn` and `print` SHALL NOT be accepted.

#### Scenario: Mundane constructs use plain keywords
- **WHEN** a program declares a function and a loop
- **THEN** the accepted syntax is `define`/`while` (not `fn`/`ritual`/`whilst`), and using a themed synonym for a mundane construct is a syntax error

#### Scenario: Legacy fn is rejected
- **WHEN** a program uses the keyword `fn`
- **THEN** the parser reports a syntax error (not a silent alias for `define`)

#### Scenario: Token positions are recorded
- **WHEN** any token is produced
- **THEN** it carries the line and column at which it began

### Requirement: Expression grammar with precedence
The parser SHALL parse expressions including `spark`/`glyph` literals, identifiers, function calls, method calls (e.g. `oracle.embed(...)`), variant construction, field access, parenthesised groups, unary `not`/negation, binary arithmetic/comparison/boolean operators with precedence (highest to lowest): unary, `* /`, `+ -`, comparison, `and`, `or`, and the builtins `speak` and `listen`.

#### Scenario: speak statement form
- **WHEN** the parser reads `speak "hello"`
- **THEN** it produces a speak statement node

#### Scenario: listen call form
- **WHEN** the parser reads `listen("> ")`
- **THEN** it produces a call to the `listen` builtin with one glyph argument
