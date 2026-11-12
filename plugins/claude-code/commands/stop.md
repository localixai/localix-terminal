---
description: Quit Localix Terminal, its window and its terminals
disable-model-invocation: true
allowed-tools: Bash(llxterm quit:*)
---

!`llxterm quit 2>&1`

Say that Localix Terminal was asked to quit. The next `llxterm window` call starts it again.
