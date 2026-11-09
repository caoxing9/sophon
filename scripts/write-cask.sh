#!/bin/sh
# Prints the Homebrew cask for a release (Apple silicon only): write-cask.sh VERSION SHA
set -eu
cat <<CASK
cask "sophon" do
  version "$1"
  sha256 "$2"

  url "https://github.com/caoxing9/sophon/releases/download/v#{version}/Sophon-#{version}-macos-aarch64.tar.gz"
  name "Sophon"
  desc "Native desktop client for coding agents"
  homepage "https://github.com/caoxing9/sophon"

  # The app updates itself (sidebar → Update); brew upgrade works too.
  auto_updates true
  depends_on arch: :arm64
  depends_on macos: ">= :sonoma"

  app "Sophon.app"

  # Signed ad hoc, not notarized yet: let Gatekeeper open it.
  postflight do
    system_command "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "#{appdir}/Sophon.app"]
  end

  zap trash: "~/.sophon"
end
CASK
