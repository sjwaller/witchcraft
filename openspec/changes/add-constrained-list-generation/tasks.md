## 1. Grammar compiler

- [ ] 1.1 Add `Grammar::List { elem, lo, hi }`; compile from bounded `Type::List`
- [ ] 1.2 Reject unbounded `Type::List` in `compile()` for divine output paths
- [ ] 1.3 Optional hi cap (lean 16) with clear diagnostic

## 2. Mock decoder

- [ ] 2.1 Generate list length in [lo,hi]; generate each element against elem grammar
- [ ] 2.2 `fallback_value` for list grammars
- [ ] 2.3 Runtime decode.rs parity for compiled artifacts

## 3. Llama GBNF

- [ ] 3.1 Expand list to length disjunction for hi ≤ cap (0..4 dungeon case first)
- [ ] 3.2 Test: generated JSON/list value respects bounds on real tokenizer (if llama feature enabled)

## 4. Frontier / falsification

- [ ] 4.1 Frontier: array schema with minItems/maxItems only on litmus-safe path
- [ ] 4.2 Falsification test: strict vs weakened list bound → distinguishable output (Mock minimum)
- [ ] 4.3 Document §8 honesty in LANGUAGE_GUIDE bounded-list section

## 5. Integration gate (independent of #5 example change)

- [ ] 5.1 Unit tests: record-with-list divine output type-checks and generates
- [ ] 5.2 `openspec validate add-constrained-list-generation --strict`

<!-- Scope note: implement this change alone to completion before enable-dungeon-master-example needs exits list. -->
