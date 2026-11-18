cask "sophon" do
  # The newest release, whichever it is: each one carries its archive
  # under this name too, so this file is not rewritten (and no commit made
  # to main) for a release. The app updates itself from there on.
  version :latest
  sha256 :no_check

  url "https://github.com/caoxing9/sophon/releases/latest/download/Sophon-macos-aarch64.tar.gz"
  name "Sophon"
  desc "Native desktop client for coding agents"
  homepage "https://github.com/caoxing9/sophon"

  # The app updates itself (sidebar → Update).
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
