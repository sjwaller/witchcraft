# language-grammar Specification

## Purpose
TBD - created by archiving change bootstrap-language-core. Update Purpose after archive.
## Requirements
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
The parser SHALL parse expressions including `spark`/`glyph` literals, identifiers, function calls, method calls (e.g. `oracle.embed(...)`), variant construction, field access, parenthesised groups, unary `not`/negation, binary arithmetic/comparison/boolean operators with precedence (highest to lowest): unary, `* /`, `+ -`, comparison, `and`, `or`, and the builtins `speak` and `listen`.

#### Scenario: speak statement form
- **WHEN** the parser reads `speak "hello"`
- **THEN** it produces a speak statement node

#### Scenario: listen call form
- **WHEN** the parser reads `listen("> ")`
- **THEN** it produces a call to the `listen` builtin with one glyph argument

#### Scenario: Arithmetic precedence
- **WHEN** the parser reads `1 + 2 * 3`
- **THEN** the AST evaluates multiplication before addition

#### Scenario: Parentheses override precedence
- **WHEN** the parser reads `(1 + 2) * 3`
- **THEN** the AST evaluates the addition first

### Requirement: Record literal expressions
The parser SHALL accept record literal expressions of the form `{ field: expr, ... }` in expression positions, including as the `fallback` expression of a `divine` block.

#### Scenario: Parse a record literal in fallback
- **WHEN** the parser reads `fallback { narration: "idle", outcome: Nothing, danger: 0 }`
- **THEN** it produces a record literal expression with three fields

#### Scenario: Record literal disambiguated from blocks
- **WHEN** `{` appears after `fallback` and is followed by an identifier and `:`
- **THEN** it is parsed as a record literal, not a statement block

### Requirement: List type declarations
The parser SHALL accept list types in type declarations:
- `list of <Type>` for a homogeneous list with no length bound, and
- `list of <lo>..<hi> of <Type>` for a list whose length is bounded inclusively between `<lo>` and `<hi>`.

#### Scenario: Parse unbounded list type
- **WHEN** the parser reads `exits: list of glyph`
- **THEN** the AST records a list type with element `glyph` and no length bound

#### Scenario: Parse bounded list type
- **WHEN** the parser reads `exits: list of 0..4 of one_of { North, South, East, West }`
- **THEN** the AST records a list type with lower bound 0, upper bound 4, and the given element type

#### Scenario: Reject bare list of without element
- **WHEN** the parser reads `list of` with no following type
- **THEN** a syntax error is reported

