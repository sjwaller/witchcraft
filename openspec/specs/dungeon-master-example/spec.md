# dungeon-master-example Specification

## Purpose
The shipped interactive example (`examples/dungeon_master.witch`) and its test harness, demonstrating the `listen` → `divine` → `enact` game-loop pattern with free narration and grammar-constrained mechanics.

## Requirements
### Requirement: Dungeon master example is well-formed
The repository SHALL ship `examples/dungeon_master.witch` that type-checks with `witch check` and runs interactively with `witch run`, using `listen` for player input, `divine` for a `Turn` output type, and `enact` for `Outcome` dispatch.

#### Scenario: Example passes check
- **WHEN** a user runs `witch check examples/dungeon_master.witch`
- **THEN** the tool exits 0

#### Scenario: Example runs with scripted stdin
- **WHEN** a user runs `witch run examples/dungeon_master.witch --seed 42` with stdin providing a sequence of player actions
- **THEN** the program completes and emits structural game output (HP lines, exit information, and a win/loss/timeout banner)

### Requirement: Interactive game pattern is documented
The LANGUAGE_GUIDE SHALL document the interactive game loop pattern (`listen` → `divine` → `enact`) using the dungeon master example, and SHALL state that narration fields (`glyph`) are free-form while mechanical fields are grammar-constrained. Documentation SHALL NOT claim model output correctness (§8).

#### Scenario: Guide references dungeon master
- **WHEN** a reader opens the interactive programs section of LANGUAGE_GUIDE
- **THEN** they find a walkthrough pointing at `examples/dungeon_master.witch`
