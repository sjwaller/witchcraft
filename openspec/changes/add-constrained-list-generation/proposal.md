## Why

**This is the on-thesis change.** Host-side list types (#3) are naming only. The dungeon master pattern requires the model to **generate** a bounded list of constrained variants (`exits: list of 0..4 of one_of { ... }`) during `divine` — not validate JSON afterward. Today `grammar.rs` rejects `Type::List` as a `divine` output; the `Grammar` enum has no list production; decoders cannot emit list-shaped values inside records. Without this, deleting the list type would not change generation — the litmus fails.

Per §4, this must convert a **runtime failure** (over-long exit list, wrong variant, unbounded repetition) into a **generation-time guarantee**: malformed lists are unreachable. Per §8, the guarantee is **shape and bound**, never that the chosen exits are good gameplay.

## What Changes

- `Grammar::List { elem, lo, hi }` compiled from bounded `list of lo..hi of T`.
- **Reject unbounded** `list of T` on `divine` output paths at compile time.
- Mock, llama (GBNF), and frontier decoders generate lists token-by-token within bounds.
- Falsification/litmus tests: weakened list bound changes generation; over-length unreachable.
- Record compilation includes list fields in generation order.

## Capabilities

### New Capabilities

- `bounded-list-generation`: grammar-constrained list production for inference outputs.

### Modified Capabilities

- `constrained-decoder`: list grammar variant; decoder contract for bounded repetition.
- `type-system`: divine output rejection of unbounded lists; bounded list in records.
- `divine-inference`: list fields in output types compile and generate.
- `grimoire-codegen`: embed list grammars in compiled artifacts (inherits grammar blob).

## Impact

- **Depends on:** `add-list-types` (surface syntax + `Type::List` bounds).
- **Independent of:** `rename-keywords`, `add-record-literals` (orthogonal; compose in #5).
- **Hard scope:** intended for implementation on a stronger model after #1–#3 land.
- Touches `grammar.rs`, `decoder.rs`, `engine/mock.rs`, `engine/llama.rs`, `engine/frontier.rs`, `witchcraft-runtime/decode.rs`, falsification harness.

## Non-goals

- List **quality** or deduplication of exits (§8).
- Unbounded lists in inference (explicitly forbidden).
- List indexing / iteration syntax in host language (defer unless needed by #5).
