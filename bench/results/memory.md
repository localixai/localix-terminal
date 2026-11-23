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
