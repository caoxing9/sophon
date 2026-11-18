cask "sophon" do
  version "0.2.37"
  sha256 "fb77fbc3cc445bd8e4ae5eb77f70686b9693322b3a01cf810f483398bd058df0"

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
