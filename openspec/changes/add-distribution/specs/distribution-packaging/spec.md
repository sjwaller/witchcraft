## ADDED Requirements

### Requirement: Prebuilt self-contained binaries per platform
The project SHALL publish prebuilt `witch` binaries (and `grimoire` once it exists) for the primary platforms: macOS (arm64, x86_64), Linux (x86_64, arm64), and Windows (x86_64). Each binary SHALL be self-contained — runnable with no Rust toolchain, no `cargo`, and no Witchcraft-specific runtime install. Linux binaries SHALL be statically linked (musl) so they do not depend on a host libc version.

#### Scenario: Downloaded binary runs without Rust
- **WHEN** a user on a supported platform downloads the release binary and runs `witch run example.witch` on a machine with no Rust installed
- **THEN** the program type-checks and executes, producing output, with no toolchain error

#### Scenario: Linux binary is portable across distributions
- **WHEN** the Linux release binary is run on a different distribution than it was built on
- **THEN** it runs without a libc/glibc version error

### Requirement: Reproducible, automated release pipeline
A tagged release (`vMAJOR.MINOR.PATCH`) SHALL trigger an automated pipeline that builds every target, runs the test suite, produces a release archive per target plus a checksum file, and publishes them as release assets. Building from source (the only step requiring Rust) SHALL be confined to this pipeline; consumers SHALL NOT need it.

#### Scenario: Tag produces published artifacts
- **WHEN** a maintainer pushes a tag matching `v*`
- **THEN** the pipeline builds all target archives with a checksums file and attaches them to the corresponding release

#### Scenario: Release tests gate publication
- **WHEN** the release pipeline runs for a tag and the test suite fails
- **THEN** no release artifacts are published

### Requirement: Documented install channels
The project SHALL provide at least two install channels: a downloadable release archive with documented manual install (extract + place on `PATH`), and a Homebrew tap (`brew install`). Each channel SHALL install only the self-contained binary; none SHALL require Rust or a compiler.

#### Scenario: Homebrew install yields a working CLI
- **WHEN** a user runs the documented `brew install` command for Witchcraft
- **THEN** `witch` is available on `PATH` and `witch --version` reports a version

#### Scenario: Archive install yields a working CLI
- **WHEN** a user extracts the release archive and places the binary on `PATH`
- **THEN** `witch run` executes a `.witch` program with no further installation

### Requirement: Version reporting
The CLI SHALL report its version via `witch --version` (and `-V`), printing the release version and the build target triple. The reported version SHALL match the published release tag.

#### Scenario: Version flag prints version and target
- **WHEN** a user runs `witch --version`
- **THEN** the CLI prints the toolchain version and the build target triple and exits 0

### Requirement: Offline first run with no configuration
A freshly installed binary SHALL run all bundled examples offline and deterministically using the built-in mock decoder, with no network access and no inference-backend configuration. Real inference backends SHALL be an optional, separately configured deployment choice, never a prerequisite of installation.

#### Scenario: Fresh install runs an example offline
- **WHEN** a user runs `witch run` on a `divine`-using example immediately after install, with no network and no backend configured
- **THEN** the program runs to completion deterministically using the bundled decoder
