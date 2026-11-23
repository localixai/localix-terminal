# The latest run

2026-11-23, llxterm 0b3fa0f with changes not yet committed, on Apple M5 (macOS 26.6.2), rustc 1.100.0, every core built with opt-level 3, fat LTO and one codegen unit.

| engine | what it is | version |
|---|---|---:|
| llxterm | this tree's core: libghostty-vt behind `llxterm::terminal::Core`, fed | ghostty-vt-sys 0.9.0+ghostty-vt.1.0.1 |
| llxterm, framed | the same, and a view's frame (`Render::frame`) after every read, as the app takes one each time it wakes | |
| alacritty_terminal | alacritty's core with its `vte` parser | 0.26.0 |
| wezterm-term | wezterm's core, from its repository | 89e01990 |
| rio-vt | rio's core without its renderer and window | 0.5.32 |
| vt100 | a small parser and screen: the floor | 0.18.1 |

## Throughput

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

## Peak memory

What a terminal holds, in MiB: the most memory a process held resident as it fed one engine one workload from a file, 4 KiB at a time, less what the same binary holds feeding nothing (its floor, the last columns).

| workload | llxterm | llxterm, framed | alacritty_terminal | wezterm-term | rio-vt | vt100 | llxterm vs alacritty_terminal | floor: engine | floor: engine-apart |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| ascii | 10.6 | 12.4 | 32.1 | 4.4 | 10.5 | 7.8 | 0.33x | 9.0 | 1.5 |
| dense | 14.7 | 17.3 | 19.4 | 75.0 | 15.6 | 20.4 | 0.76x | 9.0 | 1.5 |
| scroll-region | 0.7 | 2.4 | 3.6 | 2.1 | 1.4 | 0.2 | 0.18x | 9.0 | 1.5 |
| cursor | 2.9 | 4.6 | 9.9 | 2.0 | 3.4 | 8.7 | 0.29x | 9.0 | 1.5 |
| unicode | 12.3 | 14.2 | 35.3 | 5.1 | 11.1 | 22.3 | 0.35x | 9.0 | 1.5 |
| tui | 0.8 | 2.5 | 0.5 | 0.9 | 0.9 | 0.4 | 1.60x | 9.0 | 1.5 |
| long-line | 10.7 | 12.5 | 32.1 | 5.7 | 10.5 | 40.3 | 0.33x | 9.0 | 1.5 |
| recorded | 11.1 | 12.9 | 32.4 | 5.3 | 11.1 | 2.8 | 0.34x | 9.0 | 1.5 |
