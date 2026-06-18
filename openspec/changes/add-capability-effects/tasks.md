## 1. Surface syntax: requires, grant, capability identity

- [x] 1.1 Add tokens/keywords `requires` and `grant` (un-themed; `with` already exists); keep capability syntax `kind(param)` (e.g. `permit(escalate)`, `scope(tenant)`)
- [x] 1.2 AST: a `Capability { kind, param, span }`, `FnDecl.requires: Vec<Capability>`, and `Stmt::Grant { caps, body, span }` for `with grant <caps> { ... }`
- [x] 1.3 Parse `requires <cap>, <cap>` on a `fn` signature (after the optional return type) and the `with grant <caps> { ... }` region statement
- [x] 1.4 Update `docs/grammar.ebnf` to match the parser
- [x] 1.5 Parser tests: a `fn` with `requires`, a `with grant` region, multiple/parameterised capabilities, and a clear error on malformed capability syntax

## 2. Capability checking pass (type checker)

- [x] 2.1 Represent a capability by identity = kind + optional parameter; equality distinguishes `scope(tenant)` from `scope(user)` and from `permit(...)` (the resolution consumers inherit) <!-- Capability::same compares kind + param -->
- [x] 2.2 Thread an active capability context through statement/expression checking; a `fn` body is checked with its declared `requires` set present (the declaration grants them to the body) <!-- Checker.active_caps; check_fn seeds it with f.requires -->
- [x] 2.3 `with grant <caps> { ... }` adds capabilities to the context for the region only; they do not leak past it <!-- Stmt::Grant extends then truncates active_caps -->
- [x] 2.4 At a call site, every capability the callee `requires` MUST be in the active context, else a positioned missing-capability error naming the operation and the missing capability; requirements propagate transitively (caller grants or re-declares) <!-- check_call_caps on Expr::Call; fn_requires map built order-free -->
- [x] 2.5 Record each function's required-capability set as part of its checked signature (so requirements are visible/auditable; open question: surface in `witch check`) <!-- fn_requires map; CLI surfacing left to a later legibility pass -->
- [x] 2.6 Checker tests: missing → error (exit non-zero), granted → ok, transitive caller fails/passes, distinct kinds are distinct, grant does not leak

## 3. Runtime erasure (interpreter + lowering/codegen)

- [x] 3.1 Capabilities are compile-time only: the interpreter executes a `with grant { ... }` region as an ordinary lexical block (new scope) and ignores `requires` <!-- interp Stmt::Grant => exec_block(body) -->
- [x] 3.2 Lowering/codegen erase capabilities: a grant region lowers to its body inline; `requires` carries no runtime representation <!-- lower Stmt::Grant => lower_block(body); collect_oracles walks grant body -->
- [x] 3.3 Tests: a granted capability program runs (interpreted) and compiles/runs (compiled) with output equal to the same program with the construct removed; equivalence holds <!-- codegen tests: capabilities_are_erased_*, grant_region_compiles_to_its_body -->

## 4. Acceptance, validation, docs

- [x] 4.1 Acceptance tests covering every spec scenario (requires recorded; structured names distinct; grant in/out of region; missing/granted; transitive fail/pass; structural-not-semantic wording) <!-- crates/witchcraft/tests/lang.rs capability section + crates/witch/tests/cli.rs check_fails_on_ungranted_capability (exits non-zero) -->
- [x] 4.2 Ensure capability success wording never implies semantic correctness (§8) <!-- witch check success message already carries the structural-only caveat; asserted by check_passes_on_valid_program -->
- [x] 4.3 Update README (capability/effect discipline: `requires` + `with grant`, compile-time only, structural caveat) <!-- README "Capabilities (the compile-time effect discipline)" section -->
- [x] 4.4 `openspec validate add-capability-effects --strict`; confirm each scenario maps to a test
