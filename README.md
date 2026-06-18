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

> Linking drives a C compiler driver (`cc` by default) to produce the final
> executable; the runtime itself is carried inside `grimoire`. The driver/linker
> is a configurable seam (`GRIMOIRE_CC`, `GRIMOIRE_FUSE_LD=lld`). *Bundling* a
> linker so no system toolchain is needed at all — and the per-platform SDK
> handling that implies — is a distribution refinement.

## Build from source (contributors only)

End users do not need this. Requires a recent Rust toolchain (`cargo`).

```bash
cargo build
cargo test
cargo run -p witch -- run examples/triage.witch --seed 1
```

## Inference is a swappable engine, never a bound model

A program is written against a **NEED + POLICY**, not a model. An `oracle` names a
semantic **intent**, `divine` states the typed output (which becomes the
generation grammar), and the source states **policy** (locality, litmus-strictness)
— never a model, vendor, or engine. A deployment **manifest** binds each intent to
a concrete engine. The same program, under a different manifest, runs on the
laptop, the edge, or the cloud with **zero source change**:

```
# laptop.toml binds the intent to a local model
[need.TriageReasoner]
engine = "local"
model  = "qwen2.5-3b-instruct"
locality = "local"

[engine.local]
kind = "llama-cpp"            # or "mock" (the offline default), "anthropic", ...
gguf = "./models/qwen.gguf"
```

```
witch run triage.witch --manifest laptop.toml   # same source, local engine
witch run triage.witch --manifest cloud.toml    # same source, frontier engine
```

The language trusts the **contract**, not the engine. Every legal engine must
satisfy **grammar-by-construction**: the output type constrains generation
token-by-token, so illegal outputs are *unreachable* — never validated and
resampled. This is what lets an app outlive the models it runs on.

- **Models are named only in the manifest.** A model name in source is a design
  violation the contract forbids structurally.
- **`permit(network)` is the locality policy.** Absent ⇒ on-device-only; a network
  binding then **refuses to start** rather than silently crossing the boundary.
- **Litmus-strict by default.** An engine that cannot demonstrate token-level
  masking (e.g. a frontier provider enforcing JSON-schema server-side) is marked
  **non-litmus-safe**; binding it to a strict need refuses to start unless the
  source carries an explicit downgrade (`permit(unsafe_inference)`).
- **Load-time resolution.** Every need is resolved when the program starts; an
  unsatisfiable policy is a refusal with a diagnostic, never a silent fallback.

The offline default is a deterministic, grammar-respecting **Mock** engine (no
network), so first runs, examples, and CI are reproducible. Real engines ship
behind cargo features: `--features llama` (llama.cpp via GBNF) and
`--features frontier` (a JSON-schema API).

### The falsification test (the canary)

The thesis is real only if the type *participates in generation*. The falsification
test builds a `divine` site twice — once with the real grammar, once with the type
weakened — drives both through an engine, and asserts that at some decode step a
token the weakened grammar permitted was **forbidden** by the real grammar
(masking actually occurred). Comparing final outputs is explicitly *not* enough.
If masking cannot be shown, the test fails loudly: the engine is a wrapper, not
AI-first.

**Verified against a real model.** The litmus does not just hold for the Mock —
it holds against real **llama.cpp** weights via the GBNF grammar sampler. With a
local GGUF model (any small quantised model is enough — we test the *masking
mechanism*, not quality):

```bash
brew install cmake                       # build prereq for llama.cpp
# fetch any small GGUF into ./models, then:
WITCHCRAFT_GGUF=$PWD/models/<model>.gguf \
  cargo test -p witchcraft --features llama real_llama -- --nocapture
```

The real GBNF sampler drives a free-text token to `-inf` at the very first decode
step under the typed grammar, and the full real generation produces an *in-type*
`Record { urgency, action }` by construction while the weakened grammar wanders to
free text. That is grammar-by-construction surviving contact with a real
tokenizer.

**Non-litmus-safe engines refuse at runtime.** A frontier API enforces a schema
server-side with no observable token mask, so it is marked non-litmus-safe. A
litmus-strict `divine` bound to it **refuses to start** — proven at the CLI, not
just in a unit test:

```bash
cargo run -p witch --features frontier -- run examples/strict_divine.witch \
  --manifest examples/manifests/triage.frontier.toml
# error[runtime]: refuse to start: need `cloud-triage-v1` is litmus-strict but
# engine `frontier-large` is non-litmus-safe (...); add a source-visible
# downgrade to run anyway
```

The validate-after engine cannot silently serve a strict need; the program never
runs a line until the policy is satisfiable.

### Determinism honesty (§8)

With the **Mock** engine, the same `--seed` reproduces output exactly. With a real
local or network engine, a seed is **best-effort only** — output is not guaranteed
to reproduce across machines, quantisations, or providers. Provenance always
records the intent, resolved model, `model_version_or_sha`, backend, seed, and
sampling so a run is explainable even when it is not bit-reproducible. As always:
**shape and policy are guaranteed; quality never is.**

## Capabilities (the compile-time effect discipline)

Some operations should only be legal inside a context that has been granted the
right permission. Witchcraft expresses this as a **capability/effect discipline**
checked entirely at compile time — the substrate the forthcoming `memory` (scope)
and `familiar` (permits) primitives specialise, built once so they cannot drift
into two divergent checkers.

- A function declares what it needs with `requires`, and a `with grant` region
  makes capabilities available to the code inside it:

```
fn escalate() requires permit(escalate) { print "escalated to a human" }

with grant permit(escalate) {
    escalate()        // ok: the capability is granted here
}

escalate()            // compile error: permit(escalate) is not granted
```

- A capability is a **kind plus an optional parameter** (`permit(escalate)`,
  `scope(tenant)`), so `scope(tenant)` and `scope(user)` are distinct.
- Requirements are **transitive**: calling a `requires` function obliges the
  caller to grant the capability or re-declare the same `requires`.
- Capabilities are **erased before runtime** — they change what *compiles*, never
  what executes; a granted program runs identically to one with the scaffolding
  removed (compiled and interpreted output match).

Capability checking is **structural**: a passing check says an operation is
*permitted* in its context, never that performing it is *wise* (§8).

## Embeddings carry their space (no cross-space comparison)

An `embedding` is typed by its **vector space** (`embedding@<model>`), produced by
`oracle.embed(...)`. `similarity` and `nearest` are defined only *within* a space;
comparing embeddings from different spaces is a **compile-time error** with a
diagnostic naming both spaces — the §5.3 bug made unrepresentable. There is no
implicit cross-space bridge. (Embeddings are interpreter-only in v0.1; the
compiled ship path covers the host language plus `divine`/`enact`.)

```
oracle triage = summon "support-reasoner-v3"
let a = triage.embed("payment failed")
let b = triage.embed("card declined")
print similarity(a, b)        # same space: fine
```

A passing check guarantees only that compared embeddings share a space — never
that an embedding is meaningful or that a `nearest` result is *relevant* (§8).

## Governed memory (scope is a capability)

A `memory` is a declared, governed resource — its `scope` is bound to a capability
(via the capability discipline above), so a read or write outside the scope is a
**compile-time error**. The headline §5.2 guarantee: a cross-tenant access *will
not compile*.

```
memory tickets { scope tenant, retention 24 months, retrieval recency, audit required }

within tenant {                 # grants scope(tenant)
    tickets.write("payment failed for order 7")
    print tickets.recent(5)     # newest-first, non-expired
}

tickets.recent(5)               # compile error: scope(tenant) not granted here
```

Retention and audit are **runtime-enforced** (deterministic in-memory store with a
logical clock; expired entries are not retrieved; `audit required` records every
governed access). v0.1 ships recency/exact retrieval; semantic retrieval (a
same-space query embedding against stored embeddings) is composed in the flagship.
Memory is interpreter-only in v0.1. The green check guarantees scope adherence —
never that retained data is correct, audit complete, or retrieval relevant (§8).

## Familiars: bounded agents whose permits are checkable

A `familiar` is, deliberately, **not** a primitive — it is a named, bounded
composite of oracle/`divine`/`enact` (and memory/embedding). The elevation-worthy,
checkable thing is its `permits` set: the body is granted exactly those
capabilities and no others, so an action outside the permits **will not compile**.

```
familiar support_triage(msg) permits { invoke triage, escalate } {
    divine decision: Disposition from (msg) using triage
        with confidence >= 0.7 fallback escalate()
    enact decision.action { ... }     # only permitted actions
}
```

In v0.1 a familiar is **single-pass and deterministic** (the §10 firebreak): no
free-running loop or scheduler — an unbounded loop in a familiar body is a
compile error. Inside a familiar, `divine ... using <oracle>` requires the
`invoke <oracle>` permit. Familiars are interpreter-only in v0.1. A green check
guarantees permit adherence and bounded structure — never that the agent's plan
is sound, well-behaved, or terminates in practice (§8/§10).

## The flagship: four guarantees composed

`examples/triage_flagship.witch` is the §6.3 worked example — a tenant-scoped
`memory`, an `oracle`, a typed `Disposition`, and a single-pass `familiar` that
embeds the message, retrieves scoped history, `divine`s a decision, and `enact`s
its action. It composes all four structural guarantees and bootstrap's
discharge/exhaustiveness, with no new language feature:

```
witch run examples/triage_flagship.witch
```

Each of the paper's four "will not compile" contrasts is a negative test in
`crates/witchcraft/tests/flagship.rs`: an undischarged `divine`, an unscoped
memory read, a cross-space embedding comparison, and an out-of-permit familiar
action all fail `witch check`. The litmus also holds — deleting/weakening the
`Disposition` type changes generation under a fixed seed. A green flagship proves
the guarantees *compose*; it is never a claim that the triage decision is correct
or well-judged (§8).

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
