## Context

§6.3 is the paper's load-bearing demonstration: inference as the computation, bounded by a type, composing all four primitives. With bootstrap + the three primitive changes in place, the example becomes runnable. This change is an integration and acceptance milestone, not a feature: it composes existing constructs and pins the paper's promised compile-time failures as negative tests. Its discipline is that it may *only* compose — any gap found during integration is a defect in a primitive change, fixed there, keeping each primitive complete and coherent.

## Goals / Non-Goals

**Goals:**
- A runnable §6.3 triage program (memory + embedding + oracle + divine + enact inside a bounded familiar).
- Deterministic happy path under a fixed seed, with provenance on the enacted action.
- The four compile-error contrasts as negative tests; the litmus test; the low-confidence fallback test.
- A documented walkthrough with the §8 honesty caveat.

**Non-Goals:**
- Any new language feature or glue construct (push gaps upstream).
- Live model backends; performance.
- Semantic guarantees about triage quality (§8).

## Decisions

### D1: Compose only — gaps go upstream
The flagship adds no language feature. If `memory.nearest` output does not type-plumb into `divine` inputs, that is fixed in `add-memory-primitive` (or the relevant change), not here. **Why:** keeps primitives complete and prevents the flagship from becoming a junk drawer of glue. *Alternative:* allow small glue here — rejected; it would let primitives ship incomplete.

### D2: The example is the reduced-to-real §6.3 program
Use the paper's `Disposition` type, `tickets` memory, `triage` oracle, and `support_triage` familiar, adjusted only to v0.x scope (single-pass familiar, single-level scope, model-id embedding space). **Why:** fidelity to the source of truth while respecting each change's deferred scope.

### D3: The four contrasts are first-class acceptance tests
Each of the paper's "will not compile" cases is an explicit negative test asserting `witch check` fails with the right error. Plus the litmus test (delete the type → generation changes) and the §6.2 fault injection (low confidence → fallback). **Why:** this is the change's reason to exist — proving the thesis composes and fails safely.

### D4: Determinism via the bootstrap decoder seed
The happy path and the litmus/fallback tests run under fixed seeds using the existing deterministic grammar-respecting decoder. **Why:** reproducible acceptance; no live model.

### D5: Documentation carries the §8 caveat
The walkthrough states plainly that a green build proves the structural guarantees compose, not that the triage decisions are correct. **Why:** §8/§10.1 — the most dangerous misread is treating the green build as correctness.

## Risks / Trade-offs

- **Integration reveals composition gaps** → Expected; fix upstream per D1. The flagship surfacing gaps is a feature, not a failure.
- **The example drifts from §6.3** → Keep changes to scope-driven simplifications only; document each deviation and why.
- **Readers treat the demo as proof the system "works" (§8)** → Documentation and test names emphasise structural, not semantic, guarantees.
- **Ordering dependency risk** → This change is gated on all four predecessors; do not start until they are complete.

## Open Questions

- How faithfully can the v0.x familiar (single-pass) express the paper's `familiar support_triage`, which reads as a bounded procedure already? Confirm no persistent-loop semantics are required by the example.
- For the litmus test, what is the precise "structurally weakened type" used as the control (e.g. replace `Disposition` with an unconstrained text production) so the generated-output difference is unambiguous and stable under the seed?
- Should the flagship ship as an example program in-repo plus tests, or also as a tutorial doc? Lean: both — the example is the tutorial.
