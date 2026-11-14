cask "sophon" do
  version "0.2.19"
  sha256 "46f1561758a44a7d35f19bc7d2f742c8cfa926f3683cd88c424017edf0dfb963"

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
