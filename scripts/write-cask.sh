#!/bin/sh
# Prints the Homebrew cask for a release: write-cask.sh VERSION ARM_SHA INTEL_SHA
set -eu
cat <<CASK
cask "sophon" do
  arch arm: "aarch64", intel: "x86_64"

  version "$1"
  sha256 arm:   "$2",
         intel: "$3"

  url "https://github.com/caoxing9/sophon/releases/download/v#{version}/Sophon-#{version}-macos-#{arch}.tar.gz"
  name "Sophon"
  desc "Native desktop client for coding agents"
  homepage "https://github.com/caoxing9/sophon"

  # The app updates itself (sidebar → Update); brew upgrade works too.
  auto_updates true
  depends_on macos: ">= :sonoma"

  app "Sophon.app"

  # Signed ad hoc, not notarized yet: let Gatekeeper open it.
  postflight do
    system_command "/usr/bin/xattr", args: ["-dr", "com.apple.quarantine", "#{appdir}/Sophon.app"]
  end

  zap trash: "~/.sophon"
end
CASK
