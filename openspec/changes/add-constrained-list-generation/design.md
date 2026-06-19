## Context

The litmus (§6.3): *if you deleted the type, would the computation at the moment of inference change?* For `exits: list of 0..4 of one_of { N,S,E,W }`, deleting the bound or element type must change what the decoder **can emit** — e.g. five exits, or prose, or wrong variants become reachable.

Unbounded `list of T` in a generation grammar is **dangerous**: GBNF repetition has no natural stop without a max; validate-after would violate the thesis. **Bounded lists are the honest default.**

## Goals / Non-Goals

**Goals:**

- Compile `list of lo..hi of ElemTy` → `Grammar::List { elem, lo, hi }`.
- Generate lists during decode with length ∈ [lo, hi] and each element ∈ grammar(ElemTy).
- Compile-time error if a `divine` output type contains unbounded `list of T`.
- Litmus/falsification per engine: strict vs weakened bound produces distinguishable output.
- §8 wording in docs: shape + count bound only.

**Non-Goals:**

- Semantic constraints ("exits must be unique", "must include North if door open").
- Variable-length lists without explicit hi (use hi derived from domain size, e.g. 0..4 for four directions).
- JSON-schema `array` validate-after on frontier — must be grammar-by-construction or engine refuses (existing litmus-strict policy).

## Decisions

### D1: Surface syntax (resolved — inherits #3)

**Bounded (required for divine output):**

```witchcraft
exits: list of 0..4 of one_of { North, South, East, West }
```

- `lo..hi` inclusive on **item count**, not index values.
- For dungeon exits: `0..4` allows zero to four exits (empty list = "nowhere to go" is structurally valid; gameplay quality not guaranteed).

**Unbounded (`list of T`):** host-only (#3). **`divine` outputs MUST use bounded form** — compile error otherwise.

### D2: Grammar shape

```rust
Grammar::List {
  elem: Box<Grammar>,  // compiled element type
  lo: u32,
  hi: u32,
}
```

Generation algorithm (Mock + shared spec for real engines):

1. Choose length `n` uniformly in `[lo, hi]` (deterministic from RNG stream).
2. Emit `n` elements, each generated against `elem` grammar sequentially.
3. For `Record` containing a list field, generate fields in declaration order; list field uses List grammar.

**Litmus weaken:** compile same type with `hi` replaced by larger bound or `Text` — generation must differ.

### D3: GBNF consequence (llama.cpp)

For element grammar `G_elem`, list `lo..hi`:

```gbnf
list ::= "[]"
       | "[" elem ("," elem){0,hi-1} "]"   -- with length enforcement via custom rule
```

Practical approach: **don't use unbounded `*` in GBNF alone.** Options:

| Approach | Pros | Cons |
|---|---|---|
| **A. Expand to fixed alternation** for each length lo..hi | Litmus-safe, no validate-after | Grammar size O(hi) |
| **B. GBNF repeat with max + external length counter** | Compact | Requires llama grammar callback support |

**Lean: A for hi ≤ 16** (dungeon 0..4 is tiny). Generate disjunction:

```
list ::= empty | one | two | three | four
empty ::= "[]"
one ::= "[" elem "]"
two ::= "[" elem "," elem "]"
...
```

Element nonterminal is the compiled GBNF for `one_of { ... }`. **Open question:** hi cap in v0.x (e.g. 16) to bound grammar blow-up — document in tasks.

Frontier engine: JSON schema `array` with `minItems`/`maxItems` + `items` schema — only if engine is litmus-safe (validate-after engines already refused for strict sites).

### D4: §4 discriminator mapping

| Library approach | Native (this change) |
|---|---|
| Model emits JSON array; parse; reject if length > 4 | Length > hi unreachable during decode |
| Wrong variant string caught post-hoc | Variant not in `one_of` unreachable |
| Unbounded array crashes or truncates at runtime | Unbounded list type rejected at compile on `divine` |

Fault-injection (§6.2): compare strict `0..4` vs weakened `0..10` on same seed — output lengths must differ observably.

### D5: §8 honesty

Document explicitly: bounded list generation guarantees **cardinality and element shape**, not that the list is sensible, complete, or duplicate-free. A `0..4` exit list may be empty or `[North, North]`.

## Risks / Trade-offs

- **[Grammar explosion for large hi]** → Cap hi at 16 for v0.x; require explicit bound; dungeon uses 0..4.
- **[Frontier validate-after]** → Strict `divine` sites refuse non-litmus engines (existing policy).
- **[Empty list gameplay]** → Structurally valid; host code handles (dungeon prints exits anyway).

## Migration Plan

Land after `add-list-types`. No source migration until `enable-dungeon-master-example`. Existing programs unaffected (no list divine outputs today).

## Open Questions

1. **Maximum hi cap for grammar expansion?** **Lean:** 16, diagnostic if exceeded.
2. **Minimum lo > 0 for some domains?** Allow any lo..hi; dungeon uses 0..4.
3. **Comma-separated JSON vs native list value model?** Runtime stays `Value::List`; wire format is engine-internal.
