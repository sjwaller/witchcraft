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

## Install

Witchcraft is a first-class, standalone language: install the `witch` toolchain
and you can write, check, and run `.witch` programs with **no Rust and no other
toolchain**. (Rust is only how the maintainers build the binaries — like C is for
CPython.) Prebuilt binaries are published for macOS, Linux, and Windows.

```bash
# install script (macOS / Linux): downloads + checksum-verifies a prebuilt binary
curl -fsSL https://raw.githubusercontent.com/sjwaller/witchcraft/main/scripts/install.sh | sh

# or Homebrew
brew install sjwaller/tap/witchcraft

# or download an archive from the Releases page, extract, and put `witch` on PATH
```

Verify:

```bash
witch --version          # e.g. witch 0.1.0 (aarch64-apple-darwin)
witch run example.witch  # runs offline, deterministically, no config
```

A freshly installed binary runs every example offline using the bundled
deterministic decoder — no network and no inference backend required. Real model
backends are an optional, separate deployment choice, never an install
dependency.

## Usage

```bash
# parse + type-check only (never executes)
witch check examples/triage.witch

# type-check, then run with a fixed seed (fully reproducible)
witch run examples/triage.witch --seed 1
```

`witch check` exits non-zero on any error and never executes code. `witch run`
refuses to run an ill-typed program.

## Compile to a native executable

There are two paths, and they agree:

- **`witch run`** is the **dev loop** — a tree-walking interpreter, fast to
  iterate, no build step (the `go run` analogue).
- **`grimoire build`** is the **ship path** — it compiles a program ahead of time
  (Cranelift) and links it with the runtime into a **single self-contained native
  executable** that runs with **no Rust and no `.witch` source**.

```bash
# compile a program to a native binary
grimoire build examples/triage.witch -o triage

# run it like any other executable; --seed is accepted, just like `witch run`
./triage --seed 1
```

The compiled binary and the interpreter produce **identical** output for the same
program and seed — this equivalence is enforced in CI. Crucially, the output type
is compiled into the artifact as a **generation grammar** (the litmus property
holds in compiled form): the `divine` site stays a runtime, type-constrained call
into the bundled decoder, so inference is never pre-computed at build time.

Only the *host* language is compiled ahead of time; **inference is a runtime,
type-constrained effect** — a green compile is still structural, not semantic
(see below). `grimoire build` refuses ill-typed programs (no artifact, non-zero
exit).

> Linking currently drives the system C compiler (`cc`) to produce the final
> executable; the runtime itself is carried inside `grimoire`. Bundling a linker
> (`lld`) so no system toolchain is needed at all is a distribution refinement.

## Build from source (contributors only)

End users do not need this. Requires a recent Rust toolchain (`cargo`).

```bash
cargo build
cargo test
cargo run -p witch -- run examples/triage.witch --seed 1
```

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
  type→grammar compiler, the constrained decoder, and the lowering IR.
- `crates/witchcraft-runtime/` — the compiled runtime linked into every artifact:
  value model + reference counting, the decoder/oracle seam, provenance.
- `crates/witchcraft-codegen/` — the Cranelift backend (lowering IR → native code,
  JIT for tests and an object file for the ship path).
- `crates/witch/` — the `witch` CLI (check / run — the dev loop).
- `crates/grimoire/` — the `grimoire` CLI (build — the ship path).
- `examples/` — runnable `.witch` programs.
- `crates/witchcraft/tests/` — acceptance tests (litmus, fault injection,
  negative type tests, golden output).
- `crates/grimoire/tests/` — compiled-executable equivalence tests (compiled
  output == interpreter, ill-typed programs refused).
- `docs/grammar.ebnf` — the formal grammar, kept in step with the parser.
- `openspec/` — the specs and change proposals this implementation is built from.

## License

CC-BY-4.0 (matching the discussion paper).
