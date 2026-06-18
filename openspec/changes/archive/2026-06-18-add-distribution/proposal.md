## Why

The core distribution requirement: **once a user installs Witchcraft, they can write, check, run, build, and compile `.witch` programs without Rust or any other toolchain.** Rust is how the maintainers build `witch`/`grimoire`; it must never appear on a user's machine — exactly the model of Go, Zig, Deno, and rustc itself (built in Rust/C, shipped as a standalone binary).

Today this is *not* true in practice for one mundane reason: the project ships only source, so anyone wanting `witch` must `cargo build` it, which requires Rust. There are no prebuilt binaries, no install channel, and no versioned releases. This change closes that gap: it makes the toolchain itself a downloadable, self-contained artifact. It is packaging and release engineering, not language semantics — but without it the "no Rust" promise is unmet regardless of how good the language is.

## What Changes

- Produce **prebuilt, self-contained release binaries** of `witch` (and `grimoire` once `add-grimoire-codegen` lands) for the primary platforms: macOS (arm64 + x86_64), Linux (x86_64 + arm64), Windows (x86_64).
- Add **release automation** (CI matrix) that builds, tests, versions, checksums, and publishes those artifacts on tagged releases — the only place Rust is required.
- Provide **install channels**: a versioned release archive (`curl | tar` + PATH), a Homebrew tap (`brew install sjwaller/tap/witchcraft`), and a Windows option (scoop or zip); a one-line install script as a convenience.
- Guarantee **self-containment**: the installed binaries have no Rust/runtime prerequisites; the bundled deterministic decoder means `witch run`/compiled artifacts work fully offline out of the box.
- Add `witch --version` and a documented, reproducible release process; document that inference *backends* (Ollama/llama.cpp/API, v0.2+) are a separate, optional deployment choice — not a toolchain dependency.

**Non-goals (deferred):** the **Coven** package manager / registry (sharing and resolving `.witch` libraries and oracle/model packages is its own change); auto-update; OS-native installers (`.msi`/`.pkg`/`.deb`/`.rpm`) beyond the archive + Homebrew/scoop baseline; macOS notarization and code-signing hardening (tracked as an open question, may be a fast-follow); bundling model weights or inference engines. This change distributes the **language toolchain**, not models.

## Capabilities

### New Capabilities
- `distribution-packaging`: prebuilt cross-platform release binaries, the release-automation pipeline, the install channels, the self-containment (no-Rust, offline) guarantee, and version reporting.

### Modified Capabilities
<!-- None as delta specs: this is a new distribution-packaging capability. It packages the existing witch CLI (and later grimoire); no language-spec changes. -->

## Impact

- Depends on `bootstrap-language-core` (there is a `witch` binary to ship). It can land **early and independently** of the primitive changes: distributing the interpreter-backed `witch` already delivers Rust-free *write/check/run*.
- Pairs with `add-grimoire-codegen` to complete the set of verbs: once `grimoire build` exists, the same pipeline ships `grimoire`, delivering Rust-free *build/compile*. Until then, distribution ships `witch` (check/run) only.
- CI gains a release workflow (tag-triggered) in addition to the existing build/test workflow.
- No source or runtime change to the language; this is packaging only. Honest boundary: distribution guarantees the *toolchain* is dependency-free; it makes no claim about model quality or about the correctness of inferred values (§8).
- Build order: `bootstrap-language-core` is implemented and archived; this change adds the `distribution-packaging` capability on top of the baseline.
