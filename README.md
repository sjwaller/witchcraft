# Witchcraft

An AI-native programming language where **inference is a typed language primitive**, not a library call.

You write a program against an *inference need* and the *shape* of the answer you want back. A model fills that shape — but the type isn't checked after the model speaks; it constrains generation *as it happens*, so a value outside the type is unreachable, not merely rejected. The model lives in a deployment manifest, never in your source, so the same program runs against a tiny local model today and a better one tomorrow with **zero code changes**.

```witchcraft
oracle reader = summon "MoodReader"

type Reading = {
  feeling: one_of { Happy, Annoyed, Angry, Worried, Neutral }
  urgency: spark in 0..10
}

divine r: Reading
  from ("my payment failed again and I have a flight in an hour")
  using reader
  with confidence >= 0.0
  fallback { feeling: Neutral, urgency: 0 }

speak "feeling: ${r.feeling}, urgency: ${r.urgency}/10"
```

`urgency` can only come back as a number 0–10; `feeling` can only be one of those five — enforced *during* generation. The model named `MoodReader` is resolved from a manifest at run time.

> **Status: v0.1.** The core is proven end to end — the type genuinely constrains generation against real `llama.cpp` weights, and a compiled program runs real inference as a standalone binary. Some surface (richer agents, capability tiers) is intentionally deferred; see [Limitations](#limitations).

---

## Contents

- [Requirements](#requirements)
- [Build from source](#build-from-source)
- [Your first program](#your-first-program)
- [Running against a real model](#running-against-a-real-model)
- [Swapping models with zero code change](#swapping-models-with-zero-code-change)
- [Compiling to a standalone binary](#compiling-to-a-standalone-binary)
- [The CLI](#the-cli)
- [Language tour](#language-tour)
- [Project layout](#project-layout)
- [Limitations](#limitations)
- [The idea behind it](#the-idea-behind-it)

---

## Requirements

To **use** Witchcraft programs you only need the `witch` and `grimoire` binaries — no Rust, no Python, no other toolchain at run time.

To **build the toolchain from source** you need:

- **Rust** (stable, edition 2021) — `rustup` recommended
- **A C/C++ compiler** — only if you build with a real local model engine (`llama.cpp` is compiled from source)
- **CMake** — same; only for the `llama` engine feature
- A small **GGUF model file** — only if you want to run against a real local model (any small quantised instruct model works, e.g. a 0.5B–7B Qwen/Llama GGUF)

The default build needs none of the model dependencies — it ships a deterministic, offline **Mock** engine so you can write, check, and run programs with nothing installed but Rust.

---

## Build from source

```bash
git clone https://github.com/sjwaller/witchcraft.git
cd witchcraft

# Default build: offline, deterministic Mock engine, no model deps
cargo build --release

# The two binaries land in target/release/
#   witch     — check & run .witch programs
#   grimoire  — compile .witch programs to standalone native binaries
```

Optionally put them on your `PATH`:

```bash
export PATH="$PWD/target/release:$PATH"
witch --version
```

To build with a **real local model engine** linked in (compiles `llama.cpp`, needs CMake + a C++ compiler):

```bash
cargo build --release --features llama          # for `witch`
cargo build --release -p grimoire --features llama   # for `grimoire`
```

---

## Your first program

Create `hello.witch`:

```witchcraft
type Reading = {
  feeling: one_of { Happy, Annoyed, Angry, Worried, Neutral }
  urgency: spark in 0..10
}

oracle reader = summon "MoodReader"

divine r: Reading
  from ("the website keeps logging me out, so frustrating")
  using reader
  with confidence >= 0.0
  fallback { feeling: Neutral, urgency: 0 }

speak "feeling: ${r.feeling}"
speak "urgency: ${r.urgency}/10"
```

**Check it** (static checks only — never runs the program):

```bash
witch check hello.witch
```

**Run it** with the default offline Mock engine (deterministic, no model needed):

```bash
witch run hello.witch --seed 7
```

With no `--manifest`, every `summon` need is served by the Mock engine: it produces a value that *inhabits your type* (so the program runs and is reproducible per seed), but it does not "understand" the message. That's intentional — it lets you develop and test the *shape and logic* of a program offline, then plug in a real model when you want real answers.

---

## Running against a real model

Witchcraft programs never name a model. You bind a need to a real engine in a **manifest** (a TOML file), then pass it with `--manifest`.

1. Get a small GGUF model, e.g.:

```bash
mkdir -p models
# any small instruct GGUF works; example:
curl -L -o models/qwen2.5-0.5b-instruct-q4_k_m.gguf \
  "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf"
```

2. Create `mood.local.toml` — the model is named **only here**:

```toml
[need.MoodReader]
engine   = "local-small"
locality = "local"

[engine.local-small]
kind = "llama"
gguf = "./models/qwen2.5-0.5b-instruct-q4_k_m.gguf"
```

3. Build `witch` with the `llama` feature and run:

```bash
cargo build --release --features llama
witch run hello.witch --manifest mood.local.toml --seed 7
```

Now `MoodReader` is a real model. The output shape is still guaranteed by the type, but the *content* now reflects an actual reading of the message.

> **Seeds:** with the Mock engine a seed reproduces output exactly. With real local or network models, a seed is **best-effort** — output may vary across machines, quantisations, and providers.

---

## Swapping models with zero code change

This is the point of the language. Write a second manifest pointing at a bigger model:

```toml
# mood.better.toml
[need.MoodReader]
engine   = "local-big"
locality = "local"

[engine.local-big]
kind = "llama"
gguf = "./models/qwen2.5-7b-instruct-q4_k_m.gguf"
```

Run the **same program**, different manifest:

```bash
witch run hello.witch --manifest mood.local.toml    # small model
witch run hello.witch --manifest mood.better.toml   # bigger model — sharper reading
```

Same source. The program gets smarter because the model behind the need got smarter. A program you write today keeps working — and improves — as better models appear, with no rewrite.

### Network engines and the network permit

A manifest can also bind a need to a network model:

```toml
# mood.cloud.toml
[need.MoodReader]
engine   = "cloud"
locality = "network"

[engine.cloud]
kind  = "frontier"
model = "<your-frontier-model-id>"
```

Because reaching the network is a consequence the program author must take responsibility for, a `divine` site that may use a network engine has to **grant `permit(network)` in the source**. A program without that permit, bound to a network engine, will **refuse to start** rather than silently phone out. Network (frontier) engines that constrain output server-side (rather than by token-level masking) are marked *non-litmus-safe*: a strict need will refuse them unless the source carries an explicit, visible downgrade. See `examples/strict_divine.witch`.

---

## Compiling to a standalone binary

`grimoire` compiles a `.witch` program to a self-contained native executable.

```bash
# Mock engine (offline, no model deps)
cargo build --release -p grimoire
grimoire build hello.witch -o hello
./hello --seed 7

# With a real local model linked in
cargo build --release -p grimoire --features llama
grimoire build hello.witch -o hello
./hello --manifest mood.local.toml --seed 7
```

The produced binary is self-contained (the model engine is statically linked); the model file itself is still loaded from the path in the manifest at run time. The same compiled binary swaps engines purely by manifest, exactly like `witch run`.

---

## The CLI

### `witch` — check and run

```
witch check <file.witch>                 Static checks only; never executes.
witch run   <file.witch> [flags]          Run the program.
witch --version

Flags for `run`:
  --manifest <file.toml>   Bind each need to a real engine. Without it, the
                           deterministic offline Mock engine serves every need.
  --seed <n>               Determinism: exact with Mock; best-effort with real models.
```

`witch check` exits non-zero on any error and never runs code. It verifies *structure* — types, discharge of inferred values, exhaustive `enact`, capability/scope rules — **not** that an inferred value is correct.

### `grimoire` — compile

```
grimoire check <file.witch>              Static checks (same as `witch check`).
grimoire build <file.witch> [-o out]     Compile to a standalone native binary.
```

Build features (passed to `cargo`, not the CLI): `--features llama` links a local `llama.cpp` engine; `--features frontier` links a network engine.

---

## Language tour

A quick reference to the constructs. See `examples/` for working programs.

**Host language** — ordinary, unsurprising, deliberately *not* themed:

```witchcraft
define add(a, b) { a + b }
let name = "world"
var n = 0
while n < 3 { speak "hi ${name} ${n}"  n = n + 1 }
if n == 3 { speak "done" } else { speak "?" }
```

**Human boundary** — stdin/stdout to the responsibility-holder:

```witchcraft
speak "> "
let line = listen("")
```

**`oracle`** — declare an inference need (named, never a model):

```witchcraft
oracle reader = summon "MoodReader"
```

**Types as answer-shapes** — refined numbers, records, closed variant sets:

```witchcraft
type Action = one_of { Draft(reply: glyph), Escalate, AskClarify(question: glyph) }
type Disposition = { urgency: spark in 0..10, action: Action }
```

**`divine`** — inference *is* the computation; the type constrains generation:

```witchcraft
divine d: Disposition
  from (ticket)
  using reader
  with confidence >= 0.8      # discharge gate
  fallback "low confidence"   # taken if confidence < threshold
```

The result is `Inferred<Disposition>` until the `with confidence` gate discharges it. Using it authoritatively without discharging is a **compile error**.

**`enact`** — exhaustive dispatch over a variant set (missing/unknown arms are compile errors):

```witchcraft
enact d.action {
  Draft(reply)         => { speak "drafted: ${reply}" }
  Escalate             => { speak "escalated" }
  AskClarify(question) => { speak "asked: ${question}" }
}
```

**`memory` + `within`** — governed, scoped state; access outside the scope is a compile error:

```witchcraft
memory session:
  scope operator
  retention 1 hours
  retrieval recency

within operator {
  session.write("note")
  let recent = session.recent(3)
}
```

**`embedding`** — typed vectors carrying their space; cross-space comparison is a compile error:

```witchcraft
embedding q = reader.embed("text")
let hits = session.nearest(q, k: 5)
```

**`familiar`** — a bounded, single-pass agent with declared permits; an action outside its permits is a compile error:

```witchcraft
familiar triage(msg: glyph) permits { invoke reader, escalate } {
  # ... bounded body, no free-running loop ...
}
```

---

## Project layout

```
crates/
  witch/              # the `witch` CLI (check, run)
  grimoire/           # the `grimoire` CLI (compile to native binary)
  witchcraft/         # the language: parser, type checker, interpreter, engines
  witchcraft-codegen/ # the Cranelift native backend
  witchcraft-runtime/ # the runtime linked into compiled artifacts
examples/             # sample programs
  triage_flagship.witch   # the full example: all four primitives together
  triage.witch
  dungeon_master.witch    # interactive listen -> divine -> enact game loop
  strict_divine.witch     # demonstrates permit(network) + the strict-engine refusal
  host.witch              # plain host-language features
  manifests/              # example engine bindings (laptop/llama/cloud/frontier)
openspec/             # the spec-driven design history (proposals, specs, tasks)
```

Run the test suite:

```bash
cargo test --workspace                          # offline, Mock engine
cargo test -p witchcraft --features llama        # real-model tests (needs a GGUF; see below)
```

The real-model tests skip themselves unless you point them at a model, e.g.
`WITCHCRAFT_GGUF=./models/<model>.gguf cargo test -p witchcraft --features llama -- --nocapture`.

---

## Breaking changes (keyword rename)

| Old | New | Register |
|-----|-----|----------|
| `fn` | `define` | plain |
| `print` | `speak` | human boundary (stdout) |
| — | `listen(prompt)` | human boundary (stdin) |

There are no deprecation aliases — update source, examples, and docs mechanically.
See [docs/LANGUAGE_GUIDE.md](docs/LANGUAGE_GUIDE.md) for the naming stopping rule.

---

## Limitations

Witchcraft is a v0.1 proof of the core thesis. Known and intentional boundaries:

- **Guarantees are structural, never semantic.** The type guarantees the *shape* of an inferred value and that the engine honoured the policy. It does **not** guarantee the answer is *good* — a small model can return a well-formed but poor judgement. Choosing a capable enough model is your responsibility.
- **No free-running agents.** `familiar` is deliberately single-pass and bounded (no persistent loops, scheduling, or autonomy) as a safety boundary. Long-running behaviour is driven by a host loop you control, not by an autonomous agent.
- **Capability "tiers" are advisory only.** There is no enforced notion of "this model is good enough for this need" — that remains an open problem and is not solved here.
- **Network (frontier) engines may be non-litmus-safe.** If a provider constrains output server-side rather than by token-level masking, it cannot prove generation was constrained and is marked accordingly; strict needs refuse it without an explicit source-visible downgrade.
- **Seeds are exact only with the Mock engine.** Real models are best-effort.

---

## The idea behind it

The full argument is in the discussion paper *“Witchcraft: What Would an AI-Native Programming Language Actually Make Primitive?”* (Waller, 2025). In brief:

Most “AI-native” tooling wraps a model in a library and calls it from a language that knows nothing about the model. Witchcraft asks what it would mean to make inference, memory, embeddings, and agents *primitives* the compiler reasons about. The load-bearing test — the **litmus** — is this: *if you deleted the output type, would the computation at the moment of inference change?* For a library wrapper the type is post-hoc validation, so the answer is no. In Witchcraft the type compiles into the generation grammar and constrains the model token by token, so the answer is yes — and that difference is verified against real `llama.cpp` weights.

The language is built for the **collaboration layer**: not a language *by* AIs *for* AIs (that world needs no source code at all), but a human-authored language that makes AI a first-class primitive *so a person can still read, constrain, and answer for what the intelligence does.*

---

## License

See [LICENSE](LICENSE).
