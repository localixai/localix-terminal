#!/bin/sh
# Every place that names the version names the same one: Cargo.toml and the plugin's manifest
# (the app holds the plugin, one version for both). With a tag (v0.1.0) as the argument, the tag
# too.
set -eu
cd "$(dirname "$0")/.."

cargo=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
plugin=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' plugins/claude-code/.claude-plugin/plugin.json)

bad=0
if [ "$plugin" != "$cargo" ]; then
  echo "plugins/claude-code/.claude-plugin/plugin.json says $plugin, Cargo.toml says $cargo" >&2
  bad=1
fi
if [ $# -gt 0 ] && [ "${1#v}" != "$cargo" ]; then
  echo "the tag $1 is not v$cargo" >&2
  bad=1
fi
# A release's notes are its CHANGELOG.md section: found now, not after the builds.
if [ $# -gt 0 ] && ! grep -qx "## $cargo" CHANGELOG.md; then
  echo "CHANGELOG.md has no section \"## $cargo\"" >&2
  bad=1
fi
[ $bad -eq 0 ] && echo "version $cargo everywhere"
exit $bad
