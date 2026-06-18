## 1. Version reporting

- [x] 1.1 Add `witch --version`/`-V` printing the crate version (`CARGO_PKG_VERSION`) and the build target triple
- [x] 1.2 Capture the target triple at build time (build script or `TARGET` env) and surface it in `--version`
- [x] 1.3 CLI test: `--version` prints a non-empty version and the target triple, exits 0

## 2. Release build configuration

- [x] 2.1 Add a release profile (strip symbols, opt-level, LTO) for small self-contained binaries
- [x] 2.2 Document/declare the supported target triples (macOS arm64/x86_64, Linux x86_64/arm64 musl, Windows x86_64)
- [ ] 2.3 Verify a static musl build links cleanly (no dynamic libc dependency)

## 3. Release automation

- [x] 3.1 Add `.github/workflows/release.yml` triggered on `v*` tags with a per-target build matrix
- [x] 3.2 Run the test suite in the release job; do not publish artifacts if tests fail
- [x] 3.3 Produce a release archive per target (binary + README + LICENSE) and a `SHA256SUMS` file
- [x] 3.4 Publish archives + checksums as GitHub Release assets for the tag

## 4. Install channels

- [x] 4.1 Add `install.sh` with OS/arch detection that downloads the right archive, verifies checksum, and installs to a PATH dir
- [x] 4.2 Add a Homebrew formula (tap-ready) that installs the prebuilt binary from release archives
- [x] 4.3 Add a Windows install path (scoop manifest or documented zip install)
- [x] 4.4 Document all channels in the README (download/verify/PATH, brew, windows) with the no-Rust guarantee stated

## 5. Self-containment validation

- [x] 5.1 Confirm a built binary runs the bundled examples offline with the mock decoder and no config
- [x] 5.2 Document that real inference backends are an optional, separate deployment choice (not an install dependency)
- [ ] 5.3 Smoke-test the install script end-to-end against a draft/prerelease (or a local archive) and confirm `witch --version` works
- [x] 5.4 Run `openspec validate add-distribution --strict` and confirm every spec scenario is covered
