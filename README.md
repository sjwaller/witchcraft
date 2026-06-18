# Witchcraft

An **AI-native** programming language. Not "themed keywords over an inference
SDK" — the nativeness *is* the type system. A model call that returns a string is
sugar; Witchcraft makes inference a typed, discharged, constrained operation that
the compiler reasons about.

The defining source of truth is the discussion paper *"Witchcraft: What Would an
AI-Native Programming Language Actually Make Primitive?"* (Waller, 2025). This
repository is the v0.1 bootstrap: the smallest core that proves the thesis end to
end, deterministically and offline.

## The thesis, in one program

```
type Action = one_of {
    Draft(reply: glyph),
    Escalate,
    AskClarify(question: glyph),
}
type Disposition = { urgency: spark in 0..10, action: Action }

oracle triage = summon "mock-triage-v1"

divine decision: Disposition
    from (ticket)
    using triage
    with confidence >= 0.0
    fallback "escalated: low confidence"

enact decision.action {
    Draft(reply)        => { print "drafted: ${reply}" }
    Escalate            => { print "escalated to a human" }
    AskClarify(question) => { print "asked: ${question}" }
}
```

The **litmus test**: if you deleted the `Disposition`/`Action` types, the
computation *at the moment of inference* would change — the decoder would emit
free text instead of a bounded `urgency` and one of exactly three actions. That
is the difference between a primitive and decoration.

What the compiler guarantees here, statically:

- `urgency` can only be a `spark in 0..10` — the type compiles into the
  generation grammar, so out-of-range values are unreachable, not merely
  validated after the fact.
- `decision` is `Inferred<Disposition>` until it is **discharged** by the
  `with confidence >= θ` gate. Using it authoritatively without that gate is a
  compile error.
- `enact` must handle **exactly** the declared variants — missing or unknown
  actions are compile errors.
- provenance (oracle, model, seed) threads through to the enacted action.

## Build and run

Requires a recent Rust toolchain (`cargo`).

```bash
cargo build
cargo test

# parse + type-check only (never executes)
cargo run -p witch -- check examples/triage.witch

# type-check, then run with a fixed seed (fully reproducible)
cargo run -p witch -- run examples/triage.witch --seed 1
```

`witch check` exits non-zero on any error and never executes code. `witch run`
refuses to run an ill-typed program.

## Determinism

Inference flows through a `Decoder` seam. v0.1 ships exactly one implementation:
a deterministic, grammar-respecting `MockDecoder` that is seeded and performs no
network access. The same `--seed` always produces the same output, which is what
makes the litmus and fault-injection tests reproducible. Real model backends
implement the same trait in a later version with no caller changes.

## A green build is structural, not a correctness guarantee

This cannot be overstated (paper §8): the compiler verifies **structural**
properties — refinement bounds, the discharge rule, `enact` exhaustiveness,
variant validity. It does **not** and cannot assert that an inferred value is
*correct*. `witch check` passing means the program is well-formed around its
inference, not that the oracle was right.

## Layout

- `crates/witchcraft/` — the language: lexer, parser, type checker, interpreter,
  type→grammar compiler, and the constrained decoder.
- `crates/witch/` — the `witch` CLI.
- `examples/` — runnable `.witch` programs.
- `crates/witchcraft/tests/` — acceptance tests (litmus, fault injection,
  negative type tests, golden output).
- `docs/grammar.ebnf` — the formal grammar, kept in step with the parser.
- `openspec/` — the specs and change proposals this implementation is built from.

## License

CC-BY-4.0 (matching the discussion paper).
