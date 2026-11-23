Each workload fed 4 KiB at a time into a fresh 120 by 40 terminal with 10,000 rows of history, each engine in a process of its own; MB/s is 10^6 bytes a second, the median of 5 rounds after one round thrown away.

| workload | what it is | size |
|---|---|---:|
| ascii | numbered lines, as `seq` prints | 16.0 MiB |
| dense | every cell its own colour: truecolor text on a 256-colour ground | 16.0 MiB |
| scroll-region | lines written into a scroll region, scrolled both ways | 16.0 MiB |
| cursor | short words written all over the screen, the cursor moved each time | 16.0 MiB |
| unicode | CJK, emoji joined and with VS16, combining marks | 16.0 MiB |
| tui | full-screen frames on the other screen, each in a synchronized update | 16.0 MiB |
| long-line | lines of 64 KiB, each wrapped over 546 rows | 16.0 MiB |
| recorded | the app's recorded sessions, one after another | 16.0 MiB |

### MB/s, the median

| workload | llxterm | llxterm, framed | alacritty_terminal | wezterm-term | rio-vt | vt100 | llxterm vs alacritty_terminal | llxterm vs the fastest other |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ascii | 347 | 127 | 148 | 30 | 210 | 40 | 2.35x | 1.66x (rio-vt) |
| dense | 369 | 60 | 330 | 176 | 313 | 366 | 1.12x | 1.01x (vt100) |
| scroll-region | 619 | 97 | 175 | 50 | 541 | 148 | 3.55x | 1.14x (rio-vt) |
| cursor | 252 | 102 | 197 | 36 | 284 | 145 | 1.28x | 0.89x (rio-vt) |
| unicode | 245 | 83 | 141 | 62 | 122 | 114 | 1.74x | 1.74x (alacritty_terminal) |
| tui | 680 | 178 | 280 | 50 | 745 | 178 | 2.43x | 0.91x (rio-vt) |
| long-line | 660 | 122 | 241 | 54 | 643 | 158 | 2.73x | 1.03x (rio-vt) |
| recorded | 287 | 115 | 140 | 29 | 396 | 53 | 2.06x | 0.73x (rio-vt) |

### MB/s, the best round

| workload | llxterm | llxterm, framed | alacritty_terminal | wezterm-term | rio-vt | vt100 |
|---|---:|---:|---:|---:|---:|---:|
| ascii | 368 | 132 | 151 | 38 | 228 | 41 |
| dense | 374 | 63 | 347 | 178 | 316 | 369 |
| scroll-region | 765 | 103 | 191 | 51 | 556 | 151 |
| cursor | 255 | 103 | 203 | 37 | 286 | 148 |
| unicode | 253 | 84 | 150 | 64 | 125 | 118 |
| tui | 708 | 179 | 282 | 51 | 757 | 184 |
| long-line | 668 | 126 | 246 | 55 | 646 | 161 |
| recorded | 292 | 119 | 147 | 30 | 423 | 54 |

### Instructions retired per byte (the same on every run)

| workload | llxterm | llxterm, framed | alacritty_terminal | wezterm-term | rio-vt | vt100 |
|---|---:|---:|---:|---:|---:|---:|
| ascii | 72.1 | 214.8 | 109.6 | 586.9 | 61.3 | 587.7 |
| dense | 48.5 | 455.0 | 61.4 | 130.7 | 71.0 | 62.8 |
| scroll-region | 28.8 | 186.5 | 109.9 | 537.4 | 31.7 | 192.0 |
| cursor | 63.3 | 217.7 | 96.0 | 611.9 | 67.5 | 156.7 |
| unicode | 61.6 | 242.8 | 118.7 | 368.3 | 197.6 | 183.4 |
| tui | 33.6 | 163.9 | 93.0 | 544.1 | 36.0 | 165.5 |
| long-line | 30.4 | 213.6 | 103.3 | 523.7 | 29.9 | 187.4 |
| recorded | 82.5 | 234.4 | 111.6 | 888.9 | 64.3 | 466.0 |

### The last screen, against llxterm's

| workload | last screen |
|---|---|
| ascii | every engine ends on llxterm's screen and cursor |
| dense | every engine ends on llxterm's screen and cursor |
| scroll-region | every engine ends on llxterm's screen and cursor |
| cursor | other than llxterm's: vt100 (row 1: "c/main.rsailets     matcompilingh   Ok(())                                   mwarning passed", not "c/main.rsailets    matccompilingh   Ok(())                                   mwarning passed") |
| unicode | other than llxterm's: wezterm-term (row 38: "한국어 naïve Привет 終端機 a\u{308}\u{323} 🏳\u{fe0f}\u{200d}🌈 🏳\u{fe0f}🌈 👍🏽 👨\u{200d}👩\u{200d}👧\u{200d}👦 👍🏽 😀", not "한국어 naïve Привет 終端機 a\u{308}\u{323} 🏳\u{fe0f}\u{200d}🌈 🏳\u{fe0f}\u{200d}🌈 👍🏽 👨\u{200d}👩\u{200d}👧\u{200d}👦 👍🏽 😀") |
| tui | every engine ends on llxterm's screen and cursor |
| long-line | every engine ends on llxterm's screen and cursor |
| recorded | every engine ends on llxterm's screen and cursor |
