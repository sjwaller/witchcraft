## MODIFIED Requirements

### Requirement: Output type constrains generation
A `divine` block's declared output type SHALL be compiled into a generation grammar before inference runs. The bound engine SHALL generate a value inhabiting that grammar during decoding, including bounded list fields within records. The generated value, once discharged, SHALL be treated as the declared output type for downstream field access and `enact`.

#### Scenario: Divine produces record with list field
- **WHEN** a `divine t: Turn` site runs and discharge succeeds
- **THEN** `t.exits` is a list value whose length is within the declared bounds and whose elements inhabit the declared element type
