## 1. The composed triage program

- [x] 1.1 `examples/triage_flagship.witch`: tenant-scoped `memory tickets`, `oracle triage`, `Disposition` output type, single-pass `familiar support_triage` that embeds the message, retrieves scoped history (`within tenant`), `divine`s a `Disposition`, and `enact`s its action
- [x] 1.2 Type-checks and runs to completion under a fixed seed; the enacted action carries provenance naming the oracle, inputs, and seed; reproducible under a fixed seed

## 2. The four "will not compile" contrasts (composed)

- [x] 2.1 Undischarged divine result will not compile (discharge error)
- [x] 2.2 Unscoped memory read will not compile (out-of-scope error)
- [x] 2.3 Cross-space embedding comparison will not compile (cross-space error)
- [x] 2.4 Out-of-permit familiar action will not compile (permit violation)

## 3. Litmus, fault injection, and scope

- [x] 3.1 Litmus: deleting/weakening the `Disposition` output type changes generation under a fixed seed
- [x] 3.2 Low-confidence fault injection takes the `fallback`; the low-confidence value does not reach `enact`
- [x] 3.3 Composition adds no new language feature (only bootstrap + the primitive changes)
- [x] 3.4 Tests (`crates/witchcraft/tests/flagship.rs`, `witch run` CLI); README note that a green flagship proves the guarantees *compose*, not that triage outputs are correct (§8)
- [x] 3.5 `openspec validate integrate-triage-flagship --strict`
