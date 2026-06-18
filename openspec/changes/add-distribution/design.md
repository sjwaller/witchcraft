## Context

`bootstrap-language-core` produces a `witch` binary, but the repository ships only source, so obtaining `witch` currently means `cargo build` — i.e. installing Rust. That violates the project's stated model (config: "Rust is a BUILD-TIME dependency for maintainers ONLY"). This change makes the toolchain itself a downloadable, self-contained artifact so the user boundary is genuinely Rust-free. It is release engineering, deliberately separated from `add-grimoire-codegen` (which adds the *compile* verb) because distributing the interpreter-backed `witch` already delivers Rust-free write/check/run and need not wait for codegen.

## Goals / Non-Goals

**Goals:**
- Prebuilt, self-contained `witch` (and later `grimoire`) binaries for the primary platforms.
- Tag-triggered release automation: build → test → checksum → publish. The only place Rust runs.
- Install channels: release archive (`curl`+`tar`), Homebrew tap, a Windows option.
- Offline, no-prerequisite first run (bundled deterministic decoder).
- `witch --version` and a documented, reproducible release process.

**Non-Goals:**
- Coven package manager / registry; auto-update.
- Native OS installers beyond archive + Homebrew/scoop.
- Notarization/signing hardening (open question; possible fast-follow).
- Bundling models or inference engines.

## Decisions

### D1: Self-contained static binaries, one per target triple
Ship statically linked binaries where the platform allows it. **Linux: build against musl** (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) for a truly static, distro-independent binary. **macOS:** native arm64 + x86_64 (optionally a universal binary). **Windows:** `x86_64-pc-windows-msvc`. **Why:** "no dependencies" must include no surprise libc/glibc-version breakage; static musl is the standard way Rust CLIs achieve this.

### D2: Release automation is the only Rust touchpoint
A tag-triggered GitHub Actions matrix builds each target, runs `cargo test` (and, once it exists, the interpreter/compiled equivalence suite), produces archives + SH256SUMS, and attaches them to a GitHub Release. **Why:** centralises the Rust dependency in CI; contributors and users never need it. The existing `ci.yml` (build/test on PR) stays; this adds `release.yml` (on `v*` tags).

### D3: Install channels — archive + Homebrew first, Windows next
Priority order: (1) **release archive** + documented `curl -L … | tar xz` + PATH (works everywhere, zero infra); (2) **Homebrew tap** (`sjwaller/homebrew-tap` with a `witchcraft` formula pointing at release archives) for the primary dev audience; (3) **Windows** via scoop manifest or plain zip. A convenience **install script** (`install.sh`) wraps (1) with OS/arch detection. **Why:** archive is the universal baseline; Homebrew covers most of the likely early users; avoid committing to heavyweight native installers prematurely.

### D4: Versioning and `--version`
Single workspace version (already set in `Cargo.toml`) is the release version; tags are `vMAJOR.MINOR.PATCH`; `witch --version` (and `grimoire --version`) report it plus the build target triple. **Why:** reproducibility and bug-report clarity; one version for the whole toolchain.

### D5: The bundled mock decoder makes first run dependency-free
Because the deterministic, grammar-respecting decoder is compiled in, a freshly installed `witch` runs every example offline and reproducibly with no configuration. Real inference backends (Ollama/llama.cpp/API) are an **optional, separate deployment choice** (v0.2+), surfaced via config, not a prerequisite of installation. **Why:** keeps the "install and it just works, no Rust, no network" promise literally true.

### D6: Compiled artifacts inherit the same guarantee
An artifact produced by `grimoire build` (once that change lands) is itself a self-contained binary/module with the runtime bundled — it runs with no Rust and no `.witch` source. Distribution's self-containment guarantee therefore covers both the toolchain *and* its build outputs. (Cross-reference to `add-grimoire-codegen` D4/D5.)

## Risks / Trade-offs

- **Platform matrix maintenance cost** → keep the matrix to the five primary triples; add more only on demand.
- **macOS Gatekeeper friction without notarization** → documented workaround (`xattr -d` / right-click open) initially; notarization tracked as an open question and likely fast-follow once an Apple Developer identity exists.
- **musl edge cases** (DNS, some syscalls) → low risk for a CLI whose only network use is optional remote oracle backends; revisit if a real backend needs glibc-specific behaviour (could ship a gnu variant alongside).
- **Homebrew tap upkeep** (formula bump per release) → automate the formula update from the release workflow.
- **Binary size from a bundled runtime/decoder** → acceptable for a CLI; strip symbols in release builds.

## Open Questions

- **Signing/notarization now or fast-follow?** (Lean: ship unsigned with documented workaround first; notarize once a signing identity is available.)
- **musl-only vs musl + gnu on Linux?** (Lean: musl static as the default; add gnu only if a backend requires it.)
- **Install script trust model** — `curl | sh` convenience vs asking users to download + verify checksums explicitly. (Lean: offer both; default docs to the verifiable path.)
- **Universal macOS binary vs two arch-specific archives?** (Lean: two archives; universal only if it simplifies the Homebrew formula.)
- **Where does config for real oracle backends live** (env var, `~/.config/witchcraft`, project file) — likely owned by the v0.2 backends change, but distribution should not preclude it.
