# Homebrew formula for Witchcraft. Intended for a tap (e.g. sjwaller/homebrew-tap).
# Installs the prebuilt, self-contained `witch` binary from a GitHub release —
# no Rust or compiler required at install time.
#
# Maintenance: on each release, bump `version` and the four `sha256` values
# (these come from the published <archive>.sha256 files). The release workflow
# can be extended to open a PR that does this automatically.
class Witchcraft < Formula
  desc "AI-native programming language whose nativeness lives in the type system"
  homepage "https://github.com/sjwaller/witchcraft"
  version "0.1.0"
  license "CC-BY-4.0"

  on_macos do
    on_arm do
      url "https://github.com/sjwaller/witchcraft/releases/download/v#{version}/witch-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_DARWIN_SHA256"
    end
    on_intel do
      url "https://github.com/sjwaller/witchcraft/releases/download/v#{version}/witch-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_DARWIN_SHA256"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/sjwaller/witchcraft/releases/download/v#{version}/witch-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_LINUX_SHA256"
    end
    on_intel do
      url "https://github.com/sjwaller/witchcraft/releases/download/v#{version}/witch-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_X86_64_LINUX_SHA256"
    end
  end

  def install
    bin.install "witch-v#{version}-#{stable_target}/witch"
  end

  # Map the running platform to the archive's inner directory name.
  def stable_target
    if OS.mac?
      Hardware::CPU.arm? ? "aarch64-apple-darwin" : "x86_64-apple-darwin"
    else
      Hardware::CPU.arm? ? "aarch64-unknown-linux-musl" : "x86_64-unknown-linux-musl"
    end
  end

  test do
    assert_match "witch #{version}", shell_output("#{bin}/witch --version")
  end
end
