# The cask for the rsaryev/homebrew-tap tap, as Casks/llx-desk.rb there. A release fills in its
# version and the DMG's checksum (the line for LLX-Desk-macos.dmg in SHA256SUMS):
#   sed "s/@VERSION@/0.1.6/; s/@SHA256@/<checksum>/" packaging/homebrew/llx-desk.rb
# Then: brew install --cask rsaryev/tap/llx-desk
cask "llx-desk" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/rsaryev/llx-desk/releases/download/v#{version}/LLX-Desk-macos.dmg"
  name "LLX Desk"
  desc "Terminal for you and your coding agents"
  homepage "https://github.com/rsaryev/llx-desk"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on arch: :arm64
  depends_on macos: ">= :monterey"

  app "LLX Desk.app"
  binary "#{appdir}/LLX Desk.app/Contents/MacOS/llx-desk"

  uninstall quit: "io.github.rsaryev.llx-desk"

  zap trash: [
    "~/.config/llx-desk",
    "~/.local/bin/llx-desk",
    "~/.local/share/llx-desk",
    "~/Library/Caches/io.github.rsaryev.llx-desk",
    "~/Library/HTTPStorages/io.github.rsaryev.llx-desk",
    "~/Library/Saved Application State/io.github.rsaryev.llx-desk.savedState",
    "~/Library/WebKit/io.github.rsaryev.llx-desk",
  ]
end
