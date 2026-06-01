# Sample Homebrew formula. To publish:
#   1. Create a tap repo named `homebrew-tap` under your GitHub user.
#   2. Copy this file to `homebrew-tap/Formula/file-explorer.rb`.
#   3. Update the `url` and `sha256` after each release (the release workflow
#      attaches a `file-explorer-v0.X.Y.tar.gz` asset to every tag — compute
#      `shasum -a 256 file-explorer-v0.X.Y.tar.gz` and paste it in).
#   4. Users can then run:  brew install situmorang-com/tap/file-explorer
class FileExplorer < Formula
  desc "Translucent fuzzy-search file explorer for macOS, built in Rust with GPUI"
  homepage "https://github.com/situmorang-com/file-explorer"
  url "https://github.com/situmorang-com/file-explorer/releases/download/v0.1.0/file-explorer-v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_SHA256"
  license "MIT"
  head "https://github.com/situmorang-com/file-explorer.git", branch: "main"

  depends_on :macos
  depends_on xcode: ["15.0", :build]
  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Headless smoke test: -h would be nicer, but we don't ship CLI flags;
    # just verify the binary exists and is the right arch.
    assert_predicate bin/"file-explorer", :exist?
  end
end
