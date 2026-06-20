# The Witchcraft Language Guide

A complete, beginner-friendly tour of the language. If you've written code in
almost any language before, the ordinary parts will feel familiar; this guide
spends most of its time on the few constructs that are genuinely new.

> **The one big idea:** in Witchcraft, asking an AI model for an answer is a
> built-in language operation, and the *type* you ask for shapes what the model
> is allowed to produce *while it generates* — not a check you run afterwards.
> Everything below builds toward that.

---

## Contents

1. [Running the examples](#1-running-the-examples)
2. [The ordinary language](#2-the-ordinary-language)
   - [Values and variables](#values-and-variables)
   - [The built-in value types](#the-built-in-value-types)
   - [Functions](#functions)
   - [Control flow](#control-flow)
   - [Strings and interpolation](#strings-and-interpolation)
   - [Lists](#lists)
3. [Describing the shape of an answer: `type`](#3-describing-the-shape-of-an-answer-type)
   - [Records](#records)
   - [Refined numbers (`spark in 0..10`)](#refined-numbers)
   - [Variant sets (`one_of`)](#variant-sets-one_of)
4. [Talking to a model](#4-talking-to-a-model)
   - [`oracle` — declaring a need](#oracle--declaring-a-need)
   - [`divine` — inference as a typed operation](#divine--inference-as-a-typed-operation)
   - [The confidence gate](#the-confidence-gate)
   - [`enact` — acting on the answer](#enact--acting-on-the-answer)
5. [State and retrieval](#5-state-and-retrieval)
   - [`memory` and `within`](#memory-and-within)
   - [`embedding`](#embedding)
6. [Bounded agents: `familiar`](#6-bounded-agents-familiar)
7. [Capabilities and permits](#7-capabilities-and-permits)
8. [Putting it together: a full program](#8-putting-it-together-a-full-program)
9. [Where the model lives: manifests](#9-where-the-model-lives-manifests)
10. [Quick reference](#10-quick-reference)

---

## 1. Running the examples

Every snippet below is a real program. Save it to a file like `demo.witch` and:

```bash
witch check demo.witch              # type-check only; never runs
witch run demo.witch --seed 7       # run it (offline Mock engine by default)
```

With no manifest, a deterministic built-in **Mock** model serves every inference
request — it returns values that *fit your types* so the program runs and is
reproducible, without needing a real model installed. To use a real model, see
[section 9](#9-where-the-model-lives-manifests).

---

## 2. The ordinary language

Witchcraft's everyday syntax is deliberately plain. The unusual vocabulary
(`oracle`, `divine`, `familiar`, `speak`, `listen`) is reserved for the
intelligence seam and the human boundary; loops and variables look like loops
and variables.

#### Naming: two registers, one stopping rule

| Register | Examples | Rule |
|----------|----------|------|
| **Plain** | `define`, `let`, `var`, `while`, `if` | Universal computation — never themed |
| **Evocative** | `oracle`, `summon`, `divine`, `enact`, `speak`, `listen` | Intelligence or human-boundary seam only |

**Stopping rule:** abbreviations become plain words (the function keyword is
spelled out as `define`, never an abbreviation); plain computation keywords stay
as-is; evocative names are reserved for the seam.

#### Not object-oriented

Data is plain typed values; behaviour lives in `define` functions; encapsulation
is capabilities (`scope`, `permits`, `requires`). There are no classes and no
inheritance. If polymorphism is ever needed, the future answer is
traits/abilities — never OO inheritance. You grammar-constrain **values**, not
behaviour or objects.

### Values and variables

`let` binds a value that won't change. `var` binds one that can.

```witchcraft
let name = "Ada"       # immutable
var count = 0          # mutable
count = count + 1      # reassigning a `var` is fine
```

### The built-in value types

There are three primitive value types, plus a couple of structured ones. Two of
them have unusual names, so here's exactly what they are:

| Type     | What it is                              | Examples                    |
|----------|-----------------------------------------|-----------------------------|
| `spark`  | a number (the everyday numeric type)    | `0`, `42`, `3.5`            |
| `glyph`  | a piece of text (a string)              | `"hello"`, `"order 7"`      |
| `bool`   | true or false                           | `true`, `false`             |

**`spark` is just "number."** The name is thematic, but it behaves like the
number type in any language — you do arithmetic with it, compare it, count with
it. Whenever this guide or an error message says `spark`, read "number."

**`glyph` is just "text" (a string).** Same idea: the name is flavour, the
behaviour is an ordinary string.

You'll also meet **records** (grouped fields) and **lists** (sequences), covered
below, and the AI-specific types `oracle`, `embedding`, and `memory` in later
sections.

### Functions

```witchcraft
define add(a, b) {
    a + b              # the last expression is the return value
}

define greet(who: glyph) : glyph {   # parameter and return types are optional
    "hello ${who}"
}

speak add(2, 3)        # 5
speak greet("world")   # hello world
```

You can annotate parameters and the return type (`who: glyph`, `: glyph`), or
leave them off and let the language infer them.

### Control flow

```witchcraft
var n = 0
while n < 3 {
    speak "n = ${n}"
    n = n + 1
}

if n == 3 {
    speak "counted to three"
} else {
    speak "something is off"
}
```

The comparison operators are `< <= > >= == !=`, and boolean logic uses the words
`and`, `or`, `not`:

```witchcraft
if n > 0 and not (n == 5) {
    speak "positive and not five"
}
```

### Strings and interpolation

Text goes in double quotes. Insert any expression with `${...}`:

```witchcraft
let who = "witch"
speak "hi ${who}, ${1 + 1} times"     # hi witch, 2 times
```

### Lists

A list is a sequence of values of the same type, written in square brackets:

```witchcraft
let lines = [
    "first message",
    "second message",
    "third message"
]
```

Lists are handy for feeding a batch of inputs through inference (loop over the
list, `divine` each one).

### Human boundary: `speak` and `listen`

`speak` writes to the human on stdout (value display + newline). `listen(prompt)`
reads one line from stdin (blocking; trailing newline stripped). The prompt
argument is for your composition — it is **not** written automatically; speak the
prompt first if the user should see it:

```witchcraft
speak "> "
let action = listen("")
```

These are evocative names at the human boundary, not generic file I/O.

---

## 3. Describing the shape of an answer: `type`

This section is the hinge of the whole language. Before you ask a model for
something, you describe the **exact shape** the answer must take. That shape is
written as a `type`, and it does double duty: it documents your intent *and* it
becomes the constraint the model must satisfy.

A `type` declaration gives a name to a shape:

```witchcraft
type Temperature = spark in 0..100
```

There are three kinds of shape you'll build: **records**, **refined numbers**,
and **variant sets**.

### Records

A record groups named fields, like a struct or an object:

```witchcraft
type Reading = {
    feeling: glyph,
    urgency: spark,
    needs_human: bool
}
```

A value of type `Reading` has all three fields. You read them with a dot:
`r.feeling`, `r.urgency`, `r.needs_human`.

You can also **construct** a record value with a record literal — plain data, not
an object with methods:

```witchcraft
let reading: Reading = {
    feeling: "uneasy",
    urgency: 7,
    needs_human: true
}
```

Record literals are especially useful as `divine ... fallback { ... }` values: the
compiler checks that every field matches the declared output type before you run.

### Refined numbers

This is where types start doing real work. `spark in 0..10` means "a number,
**and** it must be between 0 and 10."

```witchcraft
type Urgency = spark in 0..10
```

When you later ask a model for an `Urgency`, the model **cannot** return `11`,
or `-1`, or `"high"`. The range is part of the type, and (as you'll see in
section 4) it constrains the model during generation. Only `spark` can be
refined this way.

### Variant sets (`one_of`)

A variant set is a closed list of named possibilities — "the answer is *one of*
exactly these." This is how you express a decision with a fixed set of outcomes.

```witchcraft
type Action = one_of {
    Draft(reply: glyph),     # a variant that carries data
    Escalate,                # a variant with no data
    AskClarify(question: glyph)
}
```

`Action` is *either* a `Draft` (carrying a `reply` text), *or* an `Escalate`
(carrying nothing), *or* an `AskClarify` (carrying a `question`). Nothing else.
When a model produces an `Action`, it must produce one of exactly these three —
it cannot invent a fourth.

You can nest these. A common pattern is a record whose fields include a refined
number and a variant set:

```witchcraft
type Disposition = {
    urgency: spark in 0..10,
    action: Action
}
```

That single type says: "the answer is a record with an urgency from 0–10 and an
action that is one of Draft / Escalate / AskClarify." That's a precise,
machine-checkable description of a triage decision — and it's about to become
the thing a model is forced to fill in.

### List types (host-side)

Homogeneous lists are written in English order, not with bracket syntax in type
positions:

```witchcraft
type Tags = list of glyph
type Exits = list of 0..4 of one_of { North, South, East, West }
```

`list of T` names an unbounded host-side list whose elements have type `T`.
`list of lo..hi of T` adds inclusive length bounds. List **values** still use
bracket literals: `[North, West]`.

#### Bounded lists as inference outputs

A list type may be a `divine` output field **only in its bounded form**:

```witchcraft
type Room = {
    exits: list of 0..4 of one_of { North, South, East, West },
    danger: spark in 0..3,
}
```

The bound is compiled into the generation grammar, so the model can only emit a
list whose length is in `[lo, hi]` and whose every element inhabits the element
type. A fifth exit, or an out-of-set direction, is **unreachable during
generation** — not trimmed afterward (the §4 discriminator, applied to lists).

An **unbounded** `list of T` as a `divine` output is a compile error: an
unbounded generation grammar has no natural stop and would force validate-after,
which the thesis forbids. Bounded is the honest default. The upper bound is
capped (16 in v0.x) to keep the generation grammar small.

**Honesty (§8).** A bounded list guarantees **shape and count bound only** —
that every element is in-type and the length is within the declared range. It
does **not** guarantee the list is sensible, complete, or duplicate-free: a
`0..4` exit list may legitimately generate as `[]` or `[North, North]`. Good
gameplay is the program's job, not the type's.

---

## 4. Talking to a model

Now the core of Witchcraft. Three constructs work together: `oracle` declares
*what* intelligence you need, `divine` *asks* for a typed answer, and `enact`
*acts* on it.

### `oracle` — declaring a need

An `oracle` is a named inference need. Crucially, **it does not name a model** —
it names a role, an intent. Which actual model fills that role is decided later,
in a manifest (section 9), never in your code.

```witchcraft
oracle triage = summon "TriageReasoner"
```

Read this as: "I need an intelligence called `triage`, whose job is
`TriageReasoner`." The string is a *need id* — a human-readable name for what
this inference is *for*. It is matched to a real model at run time.

This is what lets a Witchcraft program outlive any particular model: your code
says "I need triage reasoning," and you point that at a tiny local model today
and a far better one next year without changing a line.

### `divine` — inference as a typed operation

`divine` is the heart of the language. It means: **ask the oracle to produce a
value of a specific type, from some inputs.** Here it is in full:

```witchcraft
divine decision: Disposition
    from (message)
    using triage
    with confidence >= 0.5
    fallback escalate()
```

Reading it line by line, in plain English:

- `divine decision: Disposition` — "produce a value called `decision`, and it
  must be a `Disposition`" (that record-with-urgency-and-action type from above).
- `from (message)` — "here's the input to reason about" (you can pass several,
  comma-separated).
- `using triage` — "use the `triage` oracle to do it."
- `with confidence >= 0.5 ... fallback escalate()` — the confidence gate,
  explained just below.

Here is the part that makes Witchcraft different from "call an API and check the
result": the type `Disposition` is turned into a constraint that the model must
obey **as it generates the answer, token by token.** The model is not free to
write a paragraph and have you reject it afterwards — it is physically prevented
from producing anything that isn't a valid `Disposition`. The `urgency` *will*
be a number 0–10. The `action` *will* be one of the three declared variants.
Malformed answers aren't caught after the fact; they're unreachable.

The simple test for whether this is real: *if you deleted the type, would the
model's output change?* In Witchcraft, yes — without `Disposition`, the model is
free to produce anything, so its actual generation differs. That's the proof the
type is part of the computation, not decoration on top of it.

(What this does **not** guarantee: that the answer is *good*. The model can
return a perfectly-shaped but unwise decision. Witchcraft guarantees the shape,
never the judgement. Choosing a capable enough model is your job.)

### The confidence gate

A model's answer comes with a confidence score. The `with confidence >= θ`
clause is a gate: the value `decision` cannot be used for real until it has
*cleared* that gate.

```witchcraft
divine decision: Disposition
    from (message)
    using triage
    with confidence >= 0.5
    fallback escalate()      # taken when confidence < 0.5
```

- If the model's confidence is **≥ 0.5**, `decision` is a usable `Disposition`.
- If it's **below 0.5**, the `fallback` runs instead (here, `escalate()`).

This is enforced by the type system: until you've gated it, `decision` isn't a
`Disposition` you can act on — it's an "inferred, not-yet-discharged" value, and
trying to use it authoritatively is a **compile error**. In practice this means
low-confidence answers can't silently leak into your logic; you're forced to
decide what happens when the model isn't sure.

### `enact` — acting on the answer

When your answer is a variant set (`one_of`), `enact` dispatches on which variant
came back. You must handle **every** variant — miss one, or invent one that
doesn't exist, and it's a compile error.

```witchcraft
enact decision.action {
    Draft(reply) => {
        speak "drafted reply: ${reply}"
    }
    Escalate => {
        speak "escalated to a human"
    }
    AskClarify(question) => {
        speak "asked: ${question}"
    }
}
```

Because `Action` had exactly three variants, `enact` has exactly three arms. If
you later add a fourth variant to the type and forget to handle it here, the
compiler stops you. This is the same exhaustiveness guarantee good languages give
for their own enums — extended to a model's output.

Inside any arm you can refer to `provenance` — a record of which model, version,
and seed produced this value — useful for audit trails.

### Interactive programs: `listen` → `divine` → `enact`

The three human-and-model seams compose into the canonical interactive loop:
read a human action with `listen`, ask the model for a **typed turn** with
`divine`, and let your own code referee the result with `enact`. The shipped
example is `examples/dungeon_master.witch` — a tiny text adventure where the
model is the dungeon master.

The shape of a turn is the whole point. Narration is **free** (a `glyph` the
model improvises); every **mechanical effect is constrained** by its type:

```witchcraft
type Outcome = one_of {
  Nothing,
  Damage(amount: spark in 0..3),   # can never hit you for more than 3
  Heal(amount: spark in 0..3),
  FindItem(item: one_of { Key, Torch, Sword, Potion }),
  Victory, Death,
}

type Turn = {
  narration: glyph,                                       # free-form
  outcome:   Outcome,                                     # constrained effect
  exits:     list of 0..4 of one_of { North, South, East, West },  # at most four
  danger:    spark in 0..10,
}
```

The loop reads, divines, and dispatches:

```witchcraft
while turn_no < 12 and not won and not dead {
  let action = listen("> what do you do? ")
  divine t: Turn from (action, hp) using dm
    with confidence >= 0.0
    fallback { narration: "Nothing happens.", outcome: Nothing, exits: [North], danger: 1 }
  speak t.narration
  enact t.outcome {
    Damage(amount) => { hp = hp - amount }
    # ... one arm per outcome; the compiler enforces exhaustiveness ...
  }
  speak "  exits: ${t.exits}"          # the bounded list, printed by your code
}
```

Why this is AI-native rather than "an SDK with a prompt":

- `listen` is the **only** way text enters from the human, and `speak` the only
  way it leaves — the human boundary is explicit (§7).
- The model **cannot** hit you for 5, invent a fifth exit, or return a
  non-direction: `Damage`'s `spark in 0..3`, the bounded `list of 0..4`, and the
  closed `one_of` are masked **during generation**, not validated afterward
  (§4). Delete a bound and generation genuinely changes (§6.3).
- **Honesty (§8):** the types guarantee *shape and bound only* — never that the
  dungeon master tells a good story, picks sensible exits, or plays fair. Under
  the offline Mock engine the narration is deterministic gibberish; the
  *mechanics* are exactly as constrained as the types say. Good gameplay is a
  model-quality question the type system makes no claim about.

Run it with piped input (or interactively):

```bash
printf 'look\ngo north\nsearch\n' | witch run examples/dungeon_master.witch --seed 42
```

---

## 5. State and retrieval

### `memory` and `within`

`memory` declares a governed store: persistent state with rules about who can
touch it, how long it lives, and how it's searched.

```witchcraft
memory tickets {
    scope tenant,             # access is gated by a "tenant" capability
    retention 24 months,      # how long entries live
    retrieval recency,        # how reads are ranked (recency, semantic, or both)
    audit required            # accesses are logged
}
```

The important part is `scope`. A scoped memory can only be read or written from
inside a matching `within` block, which grants the scope:

```witchcraft
within tenant {
    tickets.write(message)        # allowed: we're inside the tenant scope
    let history = tickets.recent(5)
    speak "recalled ${history}"
}
```

Try to touch `tickets` *outside* a `within tenant { ... }` block and the program
**won't compile.** This is how Witchcraft makes "don't leak one customer's data
into another's request" a guarantee the compiler enforces, rather than a rule you
hope everyone remembers. (`mem.write(value)` adds an entry; `mem.recent(k)`
fetches the k most relevant.)

### `embedding`

An `embedding` is a numeric representation of some text, used for similarity and
retrieval. You get one by asking an oracle to embed text:

```witchcraft
embedding q = triage.embed("payment failed")
```

The interesting guarantee: an embedding carries its **space** (which model
produced it) as part of its type. Comparing two embeddings from different spaces
is a **compile error** — which catches the single most common embedding bug
(silently comparing vectors that aren't comparable and getting meaningless
numbers). You use embeddings to search a `memory`:

```witchcraft
let hits = tickets.nearest(q, k: 5)
```

---

## 6. Bounded agents: `familiar`

A `familiar` is a small, bounded "agent" — a unit that bundles some inference and
actions together, with an explicit, enforced list of what it's allowed to do.

```witchcraft
familiar support_triage(msg) permits { invoke triage, escalate } {
    # ... body ...
}
```

The `permits { ... }` clause is the point. This familiar may `invoke` the
`triage` oracle and may `escalate`, and **nothing else**. If its body tries to
perform an action it wasn't granted, the program won't compile.

Two deliberate limits, for safety and predictability:

- A familiar runs **once, start to finish** — it is not a free-running, looping,
  autonomous agent. (That's an intentional boundary; see the Limitations in the
  README.) If you want ongoing behaviour, you write a normal loop that *you*
  control and call inference inside it.
- The `permits` list is checked at compile time, so an agent's powers are
  visible in its declaration and can't be exceeded at run time.

Think of a familiar as "a function with a declared blast radius."

---

## 7. Capabilities and permits

A few constructs above (`requires`, `scope`, `permits`, `within`, `with grant`)
are all the same underlying idea: **a capability** — a named permission that some
operation needs and some region grants.

- A function can declare it `requires` a capability:
  ```witchcraft
  define escalate() requires escalate {
      speak "escalating"
  }
  ```
- A `familiar` grants its body the capabilities in its `permits` list.
- A `memory`'s `scope` is a capability; `within <scope> { ... }` grants it.
- For ad-hoc grants, `with grant <capability> { ... }` grants a capability to the
  enclosed block.

The rule is uniform: if an operation needs a capability the surrounding code
hasn't granted, it's a **compile error**. This is what makes powers — touching
scoped data, reaching the network, escalating — *visible and checkable in the
source*, which is the whole point of a language built for a human to stay in
control of what the AI parts are allowed to do.

One capability worth knowing: `permit(network)`. A `divine` that may use a
network model must be inside a region that grants `permit(network)`, so "this
program can phone out to a cloud model" is always readable in the source, never
silent.

---

## 8. Putting it together: a full program

Here is a complete, runnable triage program that uses every major construct. This
is the real flagship example shipped in the repo (`examples/triage_flagship.witch`).

```witchcraft
type Action = one_of {
    Draft(reply: glyph),
    Escalate,
    AskClarify(question: glyph)
}

type Disposition = {
    urgency: spark in 0..10,
    action: Action
}

oracle triage = summon "TriageReasoner"

memory tickets { scope tenant, retention 24 months, retrieval recency, audit required }

define escalate() requires escalate {
    speak "escalated (fallback): low confidence"
}

familiar support_triage(msg) permits { invoke triage, escalate } {
    # Embed the message; the embedding carries triage's space.
    let query = triage.embed(msg)

    # Read/write scoped history — only allowed inside `within tenant`.
    within tenant {
        tickets.write(msg)
        let history = tickets.recent(5)
        speak "recalled scoped history"
    }

    # Inference IS the computation: produce a typed Disposition. If the model
    # isn't confident enough, escalate instead of acting on a guess.
    divine decision: Disposition
        from (msg)
        using triage
        with confidence >= 0.5
        fallback escalate()

    speak "urgency: ${decision.urgency}"

    # Act on exactly the declared actions.
    enact decision.action {
        Draft(reply)         => { speak "drafted reply: ${reply}" }
        Escalate             => { speak "escalated to a human" }
        AskClarify(question) => { speak "asked: ${question}" }
    }
}

support_triage("payment failed for order 7")
```

Run it offline:

```bash
witch run examples/triage_flagship.witch --seed 7
```

---

## 9. Where the model lives: manifests

Your program named a need (`"TriageReasoner"`), never a model. A **manifest** — a
small TOML file — binds that need to a real engine. You choose the manifest at
run time:

```bash
witch run program.witch --manifest local.toml
```

A manifest for a local model:

```toml
[need.TriageReasoner]
engine   = "local"
locality = "local"

[engine.local]
kind = "llama"
gguf = "./models/qwen2.5-0.5b-instruct-q4_k_m.gguf"
```

Point the same program at a bigger model by editing only the manifest:

```toml
[engine.local]
kind = "llama"
gguf = "./models/qwen2.5-7b-instruct-q4_k_m.gguf"
```

Same source, smarter result. With **no** manifest, the deterministic Mock engine
serves every need — perfect for developing and testing offline. A network model
(`kind = "frontier"`) requires the program to grant `permit(network)`, and if the
engine can't prove it constrained generation token-by-token, a strict need will
refuse it unless the source carries an explicit downgrade.

---

## 10. Quick reference

```witchcraft
# --- ordinary ---
let x = 1                  # immutable binding
var y = 2                  # mutable binding
define f(a, b) { a + b }       # function; last expression returns
while cond { ... }         # loop
if cond { ... } else { ... }
speak "text ${expr}"       # speak to stdout (human boundary)
listen("> ")               # read one stdin line (human boundary)
[ a, b, c ]                # list literal

# --- value types ---
spark                      # number
glyph                      # text / string
bool                       # true | false

# --- answer shapes ---
type R = { f: glyph, n: spark in 0..10 }        # record + refined number
type V = one_of { A(x: glyph), B, C(y: spark) } # closed variant set

# --- inference ---
oracle o = summon "NeedName"        # declare a need (not a model)
divine v: R                          # produce a typed value...
    from (input1, input2)            # ...from these inputs
    using o                          # ...via this oracle
    with confidence >= 0.5           # gate: must clear to use authoritatively
    fallback expr                    # ...else run this
enact v.field {                      # dispatch over a variant set (exhaustive)
    A(x) => { ... }
    B    => { ... }
    C(y) => { ... }
}

# --- state & retrieval ---
memory m { scope s, retention 12 months, retrieval recency, audit required }
within s { m.write(v)  let r = m.recent(5) }
embedding e = o.embed("text")
let hits = m.nearest(e, k: 5)

# --- agents & capabilities ---
familiar a(arg) permits { invoke o, escalate } { ... }   # bounded, single-pass
define g() requires escalate { ... }                          # needs a capability
with grant permit(network) { ... }                        # grants a capability
```

---

That's the whole language. The mental model to carry away: **you describe the
shape of the answer you need, the model is constrained to fill exactly that
shape, and the model itself is a swappable part named outside your code.**
Everything else — the loops, the records, the functions — is there to wire those
typed answers into a real program.
