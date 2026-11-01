#!/bin/sh
# SessionEnd: close the tabs this project opened. Never starts the daemon, never downloads the
# program, never fails the session.
cat >/dev/null
LLX_DESK_NO_FETCH=1 "$(dirname "$0")/../bin/llx-desk" window --end >/dev/null 2>&1
exit 0
