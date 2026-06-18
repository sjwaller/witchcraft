#!/usr/bin/env sh
# Witchcraft installer. Detects OS/arch, downloads the matching release archive
# from GitHub, verifies its checksum, and installs `witch` to a bin directory.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/sjwaller/witchcraft/main/scripts/install.sh | sh
#   ./install.sh [--version vX.Y.Z] [--bin-dir DIR]
#
# No Rust or compiler required: this fetches a prebuilt, self-contained binary.

set -eu

REPO="sjwaller/witchcraft"
VERSION="latest"
BIN_DIR="${WITCH_BIN_DIR:-$HOME/.local/bin}"

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --bin-dir) BIN_DIR="$2"; shift 2 ;;
    -h|--help)
      echo "usage: install.sh [--version vX.Y.Z] [--bin-dir DIR]"; exit 0 ;;
    *) echo "error: unknown option '$1'" >&2; exit 1 ;;
  esac
done

err() { echo "error: $*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- detect platform -------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-musl" ;;
  *) err "unsupported OS '$os' (use the Windows zip from the releases page)" ;;
esac

case "$arch" in
  arm64|aarch64) arch_part="aarch64" ;;
  x86_64|amd64)  arch_part="x86_64" ;;
  *) err "unsupported architecture '$arch'" ;;
esac

target="${arch_part}-${os_part}"

# --- resolve version -------------------------------------------------------
if [ "$VERSION" = "latest" ]; then
  have curl || err "curl is required"
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
  [ -n "$VERSION" ] || err "could not determine the latest version"
fi

archive="witch-${VERSION}-${target}.tar.gz"
base="https://github.com/${REPO}/releases/download/${VERSION}"

echo "Installing witch ${VERSION} for ${target}"

# --- download + verify -----------------------------------------------------
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

have curl || err "curl is required"
curl -fsSL "${base}/${archive}" -o "${tmp}/${archive}" \
  || err "download failed: ${base}/${archive}"
curl -fsSL "${base}/${archive}.sha256" -o "${tmp}/${archive}.sha256" \
  || echo "warning: no checksum published; skipping verification" >&2

if [ -f "${tmp}/${archive}.sha256" ]; then
  echo "Verifying checksum..."
  ( cd "$tmp" && \
    if have shasum; then shasum -a 256 -c "${archive}.sha256"; \
    elif have sha256sum; then sha256sum -c "${archive}.sha256"; \
    else echo "warning: no sha256 tool; skipping" >&2; fi )
fi

# --- install ---------------------------------------------------------------
tar xzf "${tmp}/${archive}" -C "$tmp"
mkdir -p "$BIN_DIR"
install -m 0755 "${tmp}/witch-${VERSION}-${target}/witch" "${BIN_DIR}/witch"

echo "Installed witch to ${BIN_DIR}/witch"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Note: add ${BIN_DIR} to your PATH:"; echo "  export PATH=\"${BIN_DIR}:\$PATH\"" ;;
esac

"${BIN_DIR}/witch" --version || true
