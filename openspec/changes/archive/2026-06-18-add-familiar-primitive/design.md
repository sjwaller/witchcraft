## Context

§5.4 proposes the agent as a first-class schedulable entity; §5.5 immediately doubts it, suggesting `familiar` is a composite (oracle + memory + loop + tools), not a primitive — "its most exciting-sounding primitive is its least defensible." This design takes that seriously and inverts the usual framing: the defensible, elevation-worthy thing is the **permits boundary**, which is an application of the capability/effect discipline (`add-capability-effects`), not new machinery. `familiar` itself is a named, bounded composite whose only justification is making agent permissions *checkable* and behaviour *more legible than a hand-rolled loop* (§5.4). §10's "untestable autonomy" warning drives the decision to ship a single-pass, deterministic, non-scheduled construct in v0.x.

## Goals / Non-Goals

**Goals:**
- `familiar` with declared inputs, a `permits` set, and a bounded body.
- Permits bound to capabilities; out-of-permit action = compile error.
- Single-pass, deterministic execution; structural stopping conditions if the body iterates.
- Compose bootstrap's `divine`/`enact`/`oracle` and (later) memory/embedding within the boundary.

**Non-Goals:**
- Persistent/free-running loops; scheduling; concurrency; lifecycle daemons.
- Value-level permits; external tool plugins.
- Any guarantee that the agent's plan is sound, behaves well, or terminates in practice (§8/§10).

## Decisions

### D1: Familiar is a composite, and the spec says so
The construct is documented and named as a bounded composite, not a primitive. The elevation-worthy unit is `permits`. **Why:** §5.5 honesty; §10.2 ("if it reduces to an agent library wearing a costume, the costume is the only novel thing, and that is not enough"). We would rather under-claim than ship sugar dressed as a primitive.

### D2: Permits are capabilities (consume `add-capability-effects`)
A `familiar ... permits { read tickets, invoke triage, escalate }` grants exactly those capabilities to its body and declares that body's allowed actions. Any action requiring a capability not in `permits` is a compile error via the generic substrate. **Why:** one checker; the guarantee falls out of the shared mechanism. *Alternative:* a familiar-specific permit checker — rejected (drift).

### D3: Single-pass and deterministic in v0.x (the §10 firebreak)
No free-running loop, no scheduler, no concurrency. A familiar runs its bounded body once and returns; any internal iteration must have a declared, finite stopping condition. **Why:** §10 — "AI-native most easily becomes untestable autonomy." A deterministic, bounded familiar is testable; a daemon is not. Persistent familiars are a later change only after the bounded version proves its legibility claim. *Alternative:* build the §5.4 `whilst true` agent now — rejected as premature and untestable.

### D4: The legibility bar is an acceptance criterion, not a nice-to-have
Per §5.4 the familiar "earns its place only if it makes agent behaviour more legible than a hand-rolled loop, not less." The change must demonstrate the §6.2 fault-injection contrast (out-of-permit action → compile error) and a legibility comparison (the permits make the action surface explicit vs. a bare loop). **Why:** this is the whole justification; without it the construct is decoration and should be dropped.

### D5: Reuse `enact` exhaustiveness and `Inferred<T>` discharge
A familiar's actions flow through bootstrap's `enact` (exhaustive over the action variant type) and any inference uses `divine`'s discharge. **Why:** no parallel mechanisms; permits add the boundary, bootstrap provides the action/inference semantics.

## Risks / Trade-offs

- **The construct is decoration (the core risk)** → Gate on the §6.2 fault-injection test and the legibility comparison; if neither clears §5.4's bar, recommend dropping `familiar` and shipping only the `permits` annotation on plain functions.
- **Permits give false safety (§8/§10)** → Permits mean *allowed*, not *wise* or *terminating*. Strongest §8 caveat in the set; stated in spec, diagnostics, docs.
- **Pressure to add the persistent loop** → Held firmly out of scope; the bounded version must prove itself first.
- **Permit vocabulary depends on other primitives** → `read tickets` needs memory; `invoke triage` needs oracle. The mechanism works with oracle-only permits; richer tests arrive with memory/embedding.

## Open Questions

- Action-type permits only (`can it escalate at all?`) vs value-level (`escalate only to Team X`)? Lean: action-type for v0.x.
- Grant syntax and value-bound capability identity are resolved in `add-capability-effects` (D1/D4); this change consumes that resolution.
- Whether `familiar` is built as a distinct construct at all is a **GO/NO-GO gate** stated in the proposal, decided with `integrate-triage-flagship` in view (does §6.3 need a distinct construct, or does permits-on-`fn` suffice?).
