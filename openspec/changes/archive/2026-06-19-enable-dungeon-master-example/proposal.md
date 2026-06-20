## Why

`examples/dungeon_master.witch` encodes the **interactive game loop** pattern the language needs as a teaching artifact: `listen` → `divine Turn` → `enact Outcome` — free narration in `glyph`, constrained mechanics in typed fields. It currently fails to parse (ahead-of-toolchain syntax). Making it a tested, documented flagship **composes** the prior changes without adding new primitives.

## What Changes

- Fix/bring `examples/dungeon_master.witch` in line with implemented syntax (post #1–#4).
- Ensure `witch check`, interactive `witch run`, and (where applicable) `grimoire build` pass.
- Add integration test(s) for the example (deterministic Mock path + stdin scripting).
- README examples list + LANGUAGE.md section: interactive game pattern, naming philosophy, non-OO note cross-links.
- **Depends on:** `rename-keywords`, `add-record-literals`, `add-list-types`, `add-constrained-list-generation`.

## Capabilities

### New Capabilities

- `dungeon-master-example`: the shipped interactive example and its test harness.

### Modified Capabilities

- `cli-toolchain`: document dungeon master in examples list (no new subcommands).
- `triage-flagship`: no requirement changes — parallel flagship, not replacement.

## Impact

- Documentation and example only plus tests — **no new language features**.
- If #4 slips, interim version may use single `exit` field with note; full fidelity requires bounded list generation.

## Non-goals

- Real model quality for gameplay (§8).
- Manifest for `DungeonMaster` beyond Mock default (optional manifest example deferred).
