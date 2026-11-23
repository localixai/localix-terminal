#!/bin/sh
# results/latest.md: what the last run measured and on what (the date, the commit, the machine,
# the compiler, each core's version), then each benchmark's tables. run.sh calls it after the
# benchmarks; with `--quick` it gathers the smoke run's tables under target/smoke/ instead.
set -eu
cd "$(dirname "$0")"
folder=results
[ "${1:-}" = --quick ] && folder=target/smoke
machine=$( (sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m) | tr -s ' ')
system=$( (sw_vers -productName && sw_vers -productVersion) 2>/dev/null | paste -sd ' ' -)
[ -n "$system" ] || system=$(uname -sr)
commit=$(git describe --always --dirty=' with changes not yet committed' 2>/dev/null || echo '?')
# A package's version as Cargo.lock holds it.
locked() {
    awk -v name="$1" '$1 == "name" && $3 == "\"" name "\"" { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock
}
wezterm=$(grep -o 'wezterm?rev=[0-9a-f]*' Cargo.lock | head -1 | cut -d= -f2 | cut -c1-8)
{
    echo "# The latest run"
    echo
    echo "$(date +%Y-%m-%d), llxterm $commit, on $machine ($system), $(rustc --version | cut -d' ' -f1-2)," \
        "every core built with opt-level 3, fat LTO and one codegen unit."
    echo
    echo "| engine | what it is | version |"
    echo "|---|---|---:|"
    echo "| llxterm | this tree's core: libghostty-vt behind \`llxterm::terminal::Core\`, fed | ghostty-vt-sys $(locked ghostty-vt-sys) |"
    echo "| llxterm, framed | the same, and a view's frame (\`Render::frame\`) after every read, as the app takes one each time it wakes | |"
    echo "| alacritty_terminal | alacritty's core with its \`vte\` parser | $(locked alacritty_terminal) |"
    echo "| wezterm-term | wezterm's core, from its repository | $wezterm |"
    echo "| rio-vt | rio's core without its renderer and window | $(locked rio-vt) |"
    echo "| vt100 | a small parser and screen: the floor | $(locked vt100) |"
    for part in throughput memory; do
        echo
        case $part in
            throughput) echo "## Throughput" ;;
            memory) echo "## Peak memory" ;;
        esac
        echo
        cat "$folder/$part.md"
    done
} > "$folder/latest.md"
