## Context

The example as written:

```witchcraft
exits: list of one_of { North, South, East, West }   // needs bounded: 0..4
let action = listen("> what do you do? ")            // needs rename-keywords
fallback { narration: "...", outcome: Nothing, exits: [North], danger: 1 }  // needs record literals + list literal
```

Target shape after dependencies land:

```witchcraft
exits: list of 0..4 of one_of { North, South, East, West }
let action = listen("> what do you do? ")
fallback { narration: "...", outcome: Nothing, exits: [North], danger: 1 }
```

## Goals / Non-Goals

**Goals:**

- Example is canonical, copy-pasteable, and CI-tested.
- Document the pattern: human action via `listen`; DM response via `divine`; mechanics via `enact`; narration free, outcomes constrained.
- Interactive test feeds stdin lines; asserts structural output (HP, win/loss paths) under fixed seed — not narrative quality.

**Non-Goals:**

- Shipping GGUF weights or DungeonMaster manifest (Mock suffices offline).
- Multiplayer or save/load.

## Decisions

### D1: Example file is source of truth

Update `examples/dungeon_master.witch` in place — not a separate `dungeon_master_v2.witch`.

### D2: Test strategy

- **Check test:** `witch check examples/dungeon_master.witch` in CI.
- **Run test:** pipe stdin (`look\n`, `go north\n`, …) with `--seed N`; assert stdout contains structural markers (`HP:`, `exits:`, win/loss banners).
- **Optional compiled test:** `grimoire build` + run binary with same stdin (if host+I/O ABI stable).

### D3: README / LANGUAGE.md placement

New LANGUAGE.md section **"Interactive programs: listen → divine → enact"** after divine/enact tour. Include naming stopping rule box and non-OO paragraph (or cross-link to section introduced in #1).

### D4: Phased landing (if #4 delayed)

**Plumbing-only milestone** (after #1–#3): example uses `exit: one_of { ... }` single field + record fallback — runs interactively. **Full fidelity milestone** (after #4): restore `exits: list of 0..4 of ...`. Tasks.md lists both with clear gate.

## Risks / Trade-offs

- **[Mock narration is gibberish]** → Comment in example + §8 note; tests check mechanics not prose.
- **[Flaky interactive tests]** → Fixed stdin script; no TTY assumption.

## Open Questions

- Include optional `examples/manifests/dungeon.llama.toml`? **Lean:** defer; Mock-only in v0.x example.
