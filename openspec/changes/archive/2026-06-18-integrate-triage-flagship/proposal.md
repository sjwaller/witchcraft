## Why

The four primitives "are only interesting if they compose, and the composition only earns the title if inference becomes the computation itself" (§6.3). This change wires the paper's full worked example — the customer-support triage `familiar` using `oracle` + `divine` + `memory` + `embedding` — as the integration milestone, and locks the paper's four compile-error contrasts (§6.3, §8) as **negative tests**. It proves nativeness holds *at composition*, not just per-feature, and it is where the litmus test (§6.3) and the fault-injection discipline (§6.2) are demonstrated end-to-end. It introduces no new primitive: its job is to compose what exists and to fail in all the ways the paper promises.

## What Changes

- Add the `triage` example program: a `memory tickets` (tenant-scoped), an `oracle triage`, a `Disposition` output type, and a `familiar support_triage` that embeds the message, retrieves scoped history, `divine`s a `Disposition`, and `enact`s its action — the reduced-to-real §6.3 program now runnable because all four primitives exist.
- Establish the end-to-end happy path under a fixed decoder seed: deterministic run producing an enacted action with provenance.
- Lock the paper's **four compile-error contrasts as negative tests** (each MUST fail `witch check`):
  1. an undischarged `divine` result used authoritatively (bootstrap discharge rule);
  2. an unscoped read of the tenant-scoped `tickets` (memory scope);
  3. a cross-space `embedding` comparison (embedding space);
  4. the `familiar` enacting an action outside its `permits` (familiar permits).
- Demonstrate the **litmus test** (§6.3) on the triage `divine` block: deleting the `Disposition` type changes the generation.
- Demonstrate the **fault-injection** path (§6.2): a forced low-confidence result takes `fallback escalate` and does not flow downstream.

**Non-goals:** any new language feature — this change may only *compose*. If integration reveals a missing capability or type-plumbing gap, it is fixed in the relevant primitive change, not patched here. No live model backend (still the deterministic decoder). The README must keep §8 honest: a green flagship build proves the four *structural* guarantees compose, not that triage decisions are *correct*.

## Capabilities

### New Capabilities
- `triage-flagship`: the composed end-to-end behaviour of the §6.3 example and the four compile-error contrasts, the litmus demonstration, and the low-confidence fallback path, expressed as testable requirements.

### Modified Capabilities
<!-- None as delta specs. Pure composition over bootstrap + the three primitive changes; see Dependencies. -->

## Impact

- **Depends on all prior changes:** `bootstrap-language-core`, `add-capability-effects`, `add-embedding-primitive`, `add-memory-primitive`, `add-familiar-primitive`. It is the leaf of the dependency graph and should be implemented last.
- Serves as the project's acceptance suite: the single artifact that demonstrates the paper's thesis composing in one runnable program with all its promised failure modes.
- Ships documentation (the worked example walkthrough) with the explicit §8 caveat that structural ≠ semantic.
- Build order: `bootstrap-language-core` must be implemented and archived before this change's specs can become delta-aware; this change adds new capabilities until then.
