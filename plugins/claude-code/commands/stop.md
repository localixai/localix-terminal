---
description: Stop the daemon and its window
disable-model-invocation: true
allowed-tools: Bash(llx-desk:*)
---

!`llx-desk serve stop 2>&1`

Say that the daemon was asked to stop. The next `llx-desk window` call starts a fresh one.
