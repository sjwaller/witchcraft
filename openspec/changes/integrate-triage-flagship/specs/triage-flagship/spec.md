## ADDED Requirements

### Requirement: The triage example runs end-to-end
The composed §6.3 triage program — a tenant-scoped `memory tickets`, an `oracle triage`, a `Disposition` output type, and a single-pass `familiar support_triage` that embeds the message, retrieves scoped history, `divine`s a `Disposition`, and `enact`s its action — SHALL type-check and run to completion under a fixed decoder seed, producing an enacted action carrying provenance.

#### Scenario: Happy path produces an enacted action
- **WHEN** `witch run` executes the triage program for a valid message and customer under a fixed seed
- **THEN** it type-checks, runs deterministically, and enacts one of the `Disposition.action` variants with provenance naming the oracle, inputs, and seed

#### Scenario: Reproducible under a fixed seed
- **WHEN** the triage program is run twice with the same inputs and seed
- **THEN** both runs produce identical output

### Requirement: The four compile-error contrasts hold under composition
Each of the paper's "will not compile" cases SHALL be a negative test that fails `witch check` with the appropriate error, demonstrated within the composed program.

#### Scenario: Undischarged divine result will not compile
- **WHEN** the program uses the `divine` `Disposition` result authoritatively without its confidence discharge
- **THEN** `witch check` reports a discharge error and the program does not run

#### Scenario: Unscoped memory read will not compile
- **WHEN** the program reads tenant-scoped `tickets` outside a granting `within` scope
- **THEN** `witch check` reports an out-of-scope error and the program does not run

#### Scenario: Cross-space embedding comparison will not compile
- **WHEN** the program compares the query embedding against embeddings of a different space
- **THEN** `witch check` reports a cross-space error and the program does not run

#### Scenario: Out-of-permit familiar action will not compile
- **WHEN** `support_triage` attempts an action outside its declared `permits`
- **THEN** `witch check` reports a permit-violation error and the program does not run

### Requirement: The litmus test holds for the triage divine block
Deleting (or structurally weakening) the `Disposition` output type of the triage `divine` block SHALL change the generated output under a fixed seed, demonstrating that the type constrains generation rather than validating it afterward.

#### Scenario: Deleting the type changes generation
- **WHEN** the triage program is run with the `Disposition` type and again with it structurally weakened, under the same seed
- **THEN** the generated output differs (the typed run is confined to `Disposition`; the weakened run is not)

### Requirement: Low-confidence fault injection takes the fallback
When the triage `divine` yields a result below its declared confidence threshold, the program SHALL evaluate the `fallback` (escalation) and the low-confidence value SHALL NOT flow into `enact`.

#### Scenario: Forced low confidence escalates
- **WHEN** the decoder is seeded to produce a below-threshold confidence for the triage decision
- **THEN** the `fallback` escalation path runs and the low-confidence `Disposition` does not reach `enact`

### Requirement: Composition adds no new language feature
The flagship SHALL compose only existing constructs from bootstrap and the primitive changes. Any capability or type-plumbing gap discovered SHALL be resolved in the relevant primitive change, not introduced here.

#### Scenario: No new construct is defined in the flagship
- **WHEN** the flagship is implemented
- **THEN** it introduces no new language keyword, type former, or runtime construct beyond those defined by prior changes

### Requirement: Structural guarantee only
The flagship's passing build and tests SHALL be documented as proving that the four structural guarantees compose, never that the triage decisions are correct or well-judged (§8).

#### Scenario: Green flagship is not a correctness claim
- **WHEN** the flagship build and tests pass
- **THEN** the accompanying documentation states the guarantees are structural, not a claim that triage outputs are correct
