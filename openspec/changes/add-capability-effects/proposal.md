## Why

Two of the paper's primitives demand the *same* compile-time discipline: a memory access outside its declared scope must be a type error (§5.2), and a familiar acting outside its declared permits must be a type error (§5.4). The paper names this category directly — a model call is "a category of effect worth distinguishing" (§3) — and is explicit that the machinery is existing PL research, not new theory. Building this substrate once, rather than letting `add-memory-primitive` and `add-familiar-primitive` each invent their own checker, is what keeps the nativeness guarantees coherent instead of drifting into two half-implementations. This change is enabling infrastructure: it ships no user-facing primitive on its own, but makes the memory and familiar guarantees expressible and checkable.

## What Changes

- Extend the type checker with a **capability/effect discipline**: a way to declare that an operation *requires* a named capability, and that capabilities are *granted* within a bounded context.
- Introduce **capability grants**: a lexical region that grants one or more capabilities to the code within it.
- Enforce the **core guarantee**: invoking an operation that requires a capability not granted in the current context is a **compile-time error**, reported with the offending operation and the missing capability.
- Make capabilities **first-class in function signatures**: a `fn` that performs a capability-requiring operation must declare that requirement, so requirements are visible at call sites and checked transitively (a caller must itself hold or grant the capability).
- Reuse — do not reinvent — bootstrap's seams: capabilities compose with the existing `type-system` checker and `Inferred<T>`/discharge model; this change adds a new axis of static checking, not a parallel type system.

**Non-goals (deferred):** the `memory` and `familiar` primitives themselves (separate changes that *consume* this substrate); value-level capability constraints (e.g. "escalate only to Team X" — action-type granularity only here); a full effect-row polymorphism system if region scoping proves sufficient (see design open question); any runtime governance (retention, audit, scheduling) — those belong to the consuming primitives. This change adds **structural** checking only; it cannot verify a granted capability is *used wisely* (§8).

## Capabilities

### New Capabilities
- `capability-effects`: the declaration of capability requirements on operations and functions, capability grants over a lexical region, and the static rule that an ungranted required capability is a compile error. The generic mechanism that `governed-memory` (scope) and `bounded-familiar` (permits) specialise.

### Modified Capabilities
<!-- None as delta specs: bootstrap specs are not yet archived into openspec/specs/. This change's requirements are expressed as ADDED in the new capability-effects capability; it builds on bootstrap's type-system conceptually (see Impact). -->

## Impact

- Builds directly on `bootstrap-language-core` (`type-system`, `language-grammar`, `host-runtime`): adds a capability-checking pass and the grant/require syntax.
- Establishes the seam that `add-memory-primitive` (scope-as-capability) and `add-familiar-primitive` (permits-as-capability) both consume. Coherence here prevents two divergent checkers.
- No new runtime behaviour beyond what the checker needs; purely a compile-time addition plus minimal surface syntax.
- Dependency for: `add-memory-primitive`, `add-familiar-primitive`, and (transitively) `integrate-triage-flagship`.
- Build order: `bootstrap-language-core` must be implemented and archived before this change's specs can become delta-aware; this change adds new capabilities until then.
