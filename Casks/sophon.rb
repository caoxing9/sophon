cask "sophon" do
  version "0.2.2"
  sha256 "80706ec115e38c4aeb3427e399dd9a5dbda0a78e467ffd9288b81f647f358d4d"

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
