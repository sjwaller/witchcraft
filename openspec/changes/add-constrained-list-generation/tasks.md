## 1. Grammar compiler

- [x] 1.1 Add `Grammar::List { elem, lo, hi }`; compile from bounded `Type::List`
- [x] 1.2 Reject unbounded `Type::List` in `compile()` for divine output paths
- [x] 1.3 Optional hi cap (lean 16) with clear diagnostic

## 2. Mock decoder

- [x] 2.1 Generate list length in [lo,hi]; generate each element against elem grammar
- [x] 2.2 `fallback_value` for list grammars
- [x] 2.3 Runtime decode.rs parity for compiled artifacts

## 3. Llama GBNF

- [x] 3.1 Expand list to length disjunction for hi ≤ cap (0..4 dungeon case first)
- [x] 3.2 Test: generated JSON/list value respects bounds on real tokenizer (if llama feature enabled)

## 4. Frontier / falsification

- [x] 4.1 Frontier: array schema with minItems/maxItems only on litmus-safe path
- [x] 4.2 Falsification test: strict vs weakened list bound → distinguishable output (Mock minimum)
- [x] 4.3 Document §8 honesty in LANGUAGE_GUIDE bounded-list section

## 5. Integration gate (independent of #5 example change)

- [x] 5.1 Unit tests: record-with-list divine output type-checks and generates
- [x] 5.2 `openspec validate add-constrained-list-generation --strict`

<!-- Scope note: implement this change alone to completion before enable-dungeon-master-example needs exits list. -->
