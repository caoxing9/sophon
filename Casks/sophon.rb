cask "sophon" do
  version "0.2.7"
  sha256 "f7154ee522cc86d867a967c1464df3b6efa5cadb117b72dab62e76d40d883806"

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
