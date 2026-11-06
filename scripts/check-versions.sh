#!/bin/sh
# Every place that names the version names the same one: Cargo.toml, the plugin's manifest and
# the marketplace entry (the app holds the plugin, and calls one of another version old). With a
# tag (v0.1.0) as the argument, the tag too.
set -eu
cd "$(dirname "$0")/.."

cargo=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
plugin=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' plugins/claude-code/.claude-plugin/plugin.json)
market=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' .claude-plugin/marketplace.json)

bad=0
for pair in "plugins/claude-code/.claude-plugin/plugin.json=$plugin" \
  ".claude-plugin/marketplace.json=$market"; do
  if [ "${pair#*=}" != "$cargo" ]; then
    echo "${pair%%=*} says ${pair#*=}, Cargo.toml says $cargo" >&2
    bad=1
  fi
done
if [ $# -gt 0 ] && [ "${1#v}" != "$cargo" ]; then
  echo "the tag $1 is not v$cargo" >&2
  bad=1
fi
[ $bad -eq 0 ] && echo "version $cargo everywhere"
exit $bad
