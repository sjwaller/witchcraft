## Context

`add-memory-primitive` and `add-familiar-primitive` both need a compile-time guarantee of the same shape: "this operation is legal only inside a context that grants the right capability." The paper frames model-mediated operations as effects (§3) and explicitly claims no novelty in the machinery — only that the distinction is worth drawing. This change extracts that machinery once, so the two consuming primitives specialise a single checker rather than shipping two ad-hoc ones (the coherence risk flagged in bootstrap's design). It is infrastructure, not a user-facing primitive, and the proposal says so plainly.

## Goals / Non-Goals

**Goals:**
- A way to mark an operation as *requiring* a named capability.
- A `fn` can *declare* the capabilities it requires; requirements propagate to callers and are checked transitively.
- A lexical construct that *grants* capabilities to the code within it.
- The static rule: an ungranted required capability at a use site is a compile error, with a clear diagnostic.
- A representation general enough for `governed-memory` (scope) and `bounded-familiar` (permits) to specialise without reshaping callers.

**Non-Goals:**
- The memory/familiar primitives (they consume this).
- Value-level capability constraints (action-type granularity only).
- Runtime governance (retention/audit/scheduling).
- Effect *inference* across the whole program if explicit declaration is enough for v0.x.
- Any semantic guarantee that a capability is used correctly (§8: structural only).

## Decisions

### D1: Capabilities are named, declared on operations, and granted over a region
An operation (built-in or `fn`) carries a set of *required capabilities*. A grant region adds capabilities to the context for the enclosed code. At a use site, every required capability must be present in the active context or it is a compile error. **Why:** this is the minimal shape that expresses both "memory read requires the tenant scope" and "this action requires the `escalate` permit." **Grant syntax (decided, not "likely both"):** grants come in two first-class forms — (1) a **general grant region** `with grant <cap> { ... }` for explicit, ad-hoc grants, and (2) a **consuming primitive's own clause acting as a grant** — a memory's `within <value>` grants its `scope(...)`, a familiar's `permits { ... }` grants its `permit(...)` set — desugaring to the same region mechanism. The general region is the substrate; consumer clauses are ergonomic sugar over it.

### D2: Region scoping, not effect rows (decided for v0.x)
Model capabilities as a **lexical region/context** (a set of granted capability names threaded through the checker), with explicit `requires` declarations on functions — **not** a full polymorphic effect-row system. This is **decided for v0.x, not open**: region scoping is dramatically smaller and is sufficient for scope/permit checking; effect rows are a research lift we don't need. The internal capability representation is kept behind a **narrow interface** so it can later generalise to effect rows **without moving the surface syntax**. **Named reopening trigger:** the single thing that would reopen this decision is **higher-order capability passing** — capabilities crossing closures or higher-order functions; if a consumer requires that, revisit toward effect rows. Nothing else reopens it.

### D3: Requirements are explicit and transitive
A `fn` that performs a capability-requiring operation must declare `requires <cap>` in its signature; a caller must either hold the capability (be inside a grant) or re-declare the requirement. **Why:** keeps requirements visible at call sites (legibility — the §9.1 human reader), avoids whole-program inference, and makes the check local and fast. *Trade-off:* some annotation burden; acceptable and arguably desirable for an auditability-focused language.

### D4: Capability identity is kind + parameters — the resolution consumers inherit
A capability is identified by a **kind plus parameters** (e.g. `scope(tenant)`, `permit(escalate)`), so `governed-memory` and `bounded-familiar` mint their own capability kinds against one mechanism. This is the single resolution both consumers inherit, and it covers **value-bound grants**: granting a capability with a specific parameter value (e.g. entering `within customer` grants `scope(tenant)` for that customer) is expressed through the same kind+parameter identity — consumers do not invent their own grant-identity scheme. **Why:** one checker, many specialisations; prevents divergence.

### D5: Reuse bootstrap's checker and diagnostics
The capability pass runs within the existing `type-system` checker and emits positioned diagnostics in the same style; it composes with `Inferred<T>`/discharge rather than replacing anything. **Why:** one type system, one error model.

## Risks / Trade-offs

- **Region scoping turns out too weak for higher-order capability passing** → If memory/familiar need to pass capability-bearing values through closures, we revisit D2 toward effect rows. Mitigation: keep the capability representation behind a narrow interface so the checker internals can change without moving the surface syntax.
- **Annotation burden discourages use** → Keep `requires` declarations minimal and infer within a function body where unambiguous; only function boundaries need annotation.
- **Over-engineering infrastructure no primitive ends up needing exactly** → Mitigated by building the two consumers' needs concretely (scope, permit) and nothing speculative.
- **Mistaking capability-checking for safety (§8/§10)** → A granted capability means the action is *permitted*, not *wise*. Diagnostics and docs must not imply otherwise.

## Open Questions

- Should capability requirements appear in `witch check` output as part of a function's inferred signature (for legibility)? Lean: yes, surface them.
