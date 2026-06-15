## ADDED Requirements

### Requirement: Un-themed host syntax
The lexer and parser SHALL provide plain, conventional keywords for mundane constructs and SHALL NOT theme them: `fn` (function), `let` (immutable binding), `var` (mutable binding), `while`, `if`/`else`, and `print`. Occult vocabulary SHALL be reserved for constructs that name something genuinely new to the language (`oracle`, `summon`, `divine`, `enact`, `fallback`). Tokens SHALL carry source position (line and column).

#### Scenario: Mundane constructs use plain keywords
- **WHEN** a program declares a function and a loop
- **THEN** the accepted syntax is `fn`/`while` (not `ritual`/`whilst`), and using a themed synonym for a mundane construct is a syntax error

#### Scenario: Token positions are recorded
- **WHEN** any token is produced
- **THEN** it carries the line and column at which it began

### Requirement: Type declaration grammar
The parser SHALL accept type declarations introduced by `type`, including: record types `{ field: Type, ... }`, sum/variant types `one_of { A, B(field: Type), ... }`, and refinement types such as `spark in 0..10`. Type names `spark` (numeric) and `glyph` (text) SHALL be recognised primitives.

#### Scenario: Parse a record with a variant field
- **WHEN** the parser reads `type Disposition = { urgency: spark in 0..10, action: one_of { Draft(reply: glyph), Escalate } }`
- **THEN** it produces a record type whose `urgency` field is a refined `spark` and whose `action` field is a sum type with variants `Draft` (carrying a `glyph`) and `Escalate`

#### Scenario: Refinement bound is captured
- **WHEN** the parser reads `spark in 0..10`
- **THEN** the AST records the refinement with lower bound 0 and upper bound 10

### Requirement: divine and enact clause grammar
The parser SHALL accept a `divine` block with a declared output type and the clauses `from (<inputs>)`, `using <oracle>`, `with confidence >= <threshold>`, and `fallback <expr>`. It SHALL accept an `enact <expr>` statement. Omitting the `with confidence`/`fallback` discharge SHALL be permitted by the grammar (the type system, not the parser, enforces discharge).

#### Scenario: Parse a divine block
- **WHEN** the parser reads a `divine decision: Disposition from (msg) using triage with confidence >= 0.80 fallback escalate(msg)` block
- **THEN** it produces a divine node with output type `Disposition`, inputs `(msg)`, oracle `triage`, threshold `0.80`, and a fallback expression

#### Scenario: Missing clause is a syntax error
- **WHEN** a `divine` block omits the `using <oracle>` clause
- **THEN** the parser reports a syntax error identifying the missing clause and its position

### Requirement: Expression grammar with precedence
The parser SHALL parse expressions including `spark`/`glyph` literals, identifiers, function calls, method calls (e.g. `oracle.invoke(...)`), variant construction, field access, parenthesised groups, unary `not`/negation, and binary arithmetic/comparison/boolean operators with precedence (highest to lowest): unary, `* /`, `+ -`, comparison, `and`, `or`.

#### Scenario: Arithmetic precedence
- **WHEN** the parser reads `1 + 2 * 3`
- **THEN** the AST evaluates multiplication before addition

#### Scenario: Parentheses override precedence
- **WHEN** the parser reads `(1 + 2) * 3`
- **THEN** the AST evaluates the addition first
