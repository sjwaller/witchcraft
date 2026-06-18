## Why

The agent — Witchcraft's `familiar` (§5.4) — is the paper's most speculative candidate, and §5.5 turns the method on it: a `familiar` "may not be a primitive at all but a composite — an orchestration of the other three plus tools," in which case "elevating it is a category error: it is the integral, not a new unit." This proposal accepts that verdict. We do **not** claim `familiar` is a primitive. What genuinely earns elevation is the **bounded capability boundary** (`permits`): a declared permission set the compiler enforces, so a familiar cannot act outside what it was granted. That boundary is an application of the existing capability/effect discipline (§3), not new theory — and the only honest justification for shipping `familiar` is §5.4's bar: it must make agent behaviour **more legible than a hand-rolled loop**. If it fails that bar it should not exist; users can write a `permits`-annotated `fn` over `while` + `divine` instead.

## Go / No-Go gate

This change builds the `familiar` keyword **only if** the flagship requires a bounded-but-iterating construct that a `permits`-annotated `fn` cannot express. Otherwise `familiar` reduces to a `permits` annotation on `fn`, and this change **merges into `add-capability-effects`** (which already owns `permits`) rather than shipping a separate construct. The gate is decided **with `integrate-triage-flagship` in view**: does the §6.3 triage program actually need a distinct `familiar` construct, or does permits-on-`fn` suffice? Per §5.5, choosing *not* to build a separate construct is a legitimate — and possibly the correct — outcome, not a failure.

## What Changes

- Introduce `familiar` as a **bounded, named composite** (explicitly not a primitive): a `fn`-like construct that declares a `permits` set and runs its body within that boundary.
- Bind `permits` to **capabilities** (via `add-capability-effects`): the familiar's body is type-checked against its declared permits; performing an action it does not permit is a **compile-time error**.
- Enforce the **native guarantee**: a familiar cannot `enact` (or otherwise perform) an action outside its declared `permits` — the paper's "the familiar cannot enact an action outside its declared permits."
- Scope v0.x to a **single-pass, deterministic bounded procedure** — no free-running loop, no scheduling, no concurrency. A familiar declares its inputs, permits, and a bounded body, and returns.
- Require declared **stopping conditions** structurally where a bounded body could iterate (kept finite/deterministic for testability).

**Non-goals (explicitly deferred, by §10 caution):** persistent/free-running familiars (`whilst true ... monitor()`); scheduling/concurrency/lifecycle management ("untestable autonomy" is precisely what §10 warns against); value-level permits (e.g. "escalate only to Team X" — action-type granularity only); tool plugins beyond the existing primitives. Honest boundary: the compiler enforces *permits and structural bounds*, never that the agent's **plan is sound, that it behaves well, or that it terminates in practice** (§8, §10) — the loudest such caveat of any change in this set.

## Capabilities

### New Capabilities
- `bounded-familiar`: the `familiar` construct (declared inputs + `permits` + bounded body), permit-as-capability enforcement (out-of-permit action = compile error), and structural stopping-condition requirements. Specialises `capability-effects` for the permits guarantee. Framed throughout as a safety-bounded composite, not a primitive.

### Modified Capabilities
<!-- None as delta specs (bootstrap/capability-effects not yet archived). Consumes capability-effects; composes oracle/divine/enact from bootstrap; see Impact. -->

## Impact

- Depends on `bootstrap-language-core` (`divine`/`enact`/`oracle`, `type-system`, `host-runtime`) and on `add-capability-effects` (permits = capability).
- Mostly independent of `add-memory-primitive`/`add-embedding-primitive`, but its permit vocabulary (`read tickets`, `invoke triage`) and negative tests become richer once those exist; the flagship exercises the full set.
- Required by `integrate-triage-flagship` (the triage `familiar`).
- This change carries the project's highest "looks impressive, may be decoration" risk; the §6.2 fault-injection test (an out-of-permit action that will not compile) IS its justification and must pass, or the construct should be dropped.
- Build order: `bootstrap-language-core` must be implemented and archived before this change's specs can become delta-aware; this change adds new capabilities until then.
