# The cask for the rsaryev/homebrew-tap tap, as Casks/llxterm.rb there. A release fills in its
# version and the DMG's checksum (the line for Localix-Terminal-macos.dmg in SHA256SUMS):
#   sed "s/@VERSION@/0.1.6/; s/@SHA256@/<checksum>/" packaging/homebrew/llxterm.rb
# Then: brew install --cask rsaryev/tap/llxterm
cask "llxterm" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/rsaryev/localix-terminal/releases/download/v#{version}/Localix-Terminal-macos.dmg"
  name "Localix Terminal"
  desc "Terminal for you and your coding agents"
  homepage "https://github.com/rsaryev/localix-terminal"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on arch: :arm64
  depends_on macos: ">= :monterey"

  app "Localix Terminal.app"
  binary "#{appdir}/Localix Terminal.app/Contents/MacOS/llxterm"

  uninstall quit: "io.github.rsaryev.localix-terminal"

  zap trash: [
    "~/.config/llxterm",
    "~/.local/bin/llxterm",
    "~/.local/share/llxterm",
    "~/Library/Caches/io.github.rsaryev.localix-terminal",
    "~/Library/HTTPStorages/io.github.rsaryev.localix-terminal",
    "~/Library/Saved Application State/io.github.rsaryev.localix-terminal.savedState",
    "~/Library/WebKit/io.github.rsaryev.localix-terminal",
  ]
end
