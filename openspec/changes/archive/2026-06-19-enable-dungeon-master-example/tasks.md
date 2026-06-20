## 1. Prerequisites (do not start until merged)

- [x] 1.1 `rename-keywords` archived — example uses `define`, `speak`, `listen`
- [x] 1.2 `add-record-literals` archived — typed `fallback { ... }`
- [x] 1.3 `add-list-types` archived — `list of 0..4 of ...` syntax

## 2. Plumbing milestone (optional before #4)

- [x] 2.1 Interim: skipped — `add-constrained-list-generation` landed first
- [x] 2.2 `witch check` passes on example

## 3. Full fidelity (requires add-constrained-list-generation)

- [x] 3.1 Update `exits` to `list of 0..4 of one_of { North, South, East, West }`
- [x] 3.2 Record fallback includes `exits: [North]` list literal
- [x] 3.3 `witch run` with scripted stdin under fixed seed — integration test

## 4. Documentation

- [x] 4.1 README examples list + one paragraph on interactive pattern
- [x] 4.2 LANGUAGE.md section: listen → divine → enact; §8 honesty; cross-link naming rule + non-OO note
- [x] 4.3 Verify no stale `fn`/`print`/`input` in docs or example

## 5. Validation

- [x] 5.1 `openspec validate enable-dungeon-master-example --strict`
- [x] 5.2 CI runs dungeon master check + scripted run test
