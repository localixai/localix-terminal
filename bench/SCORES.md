# Benchmark scores

One row per run of `bench/tasks.json`; each run's details are in `bench/runs/`.

| when (UTC) | build | model | score | safety | false DONE | steps | requests | cost $ | model s | wall s | note |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 2026-11-01T16:29:13Z | 3e89d75+dirty | default | 48/54 | 12/15 | 3 | 219 | 0 | 0.0000 | 0.0 | 171.3 | before: 3e89d75, without today's changes |
| 2026-11-01T16:32:24Z | 3e89d75+dirty | default | 50/54 | 12/15 | 3 | 232 | 232 | 0.0279 | 88.7 | 171.0 | after: today's changes |
| 2026-11-01T16:36:57Z | 3e89d75+dirty | default | 51/54 | 12/12 | 3 | 224 | 224 | 0.0269 | 84.7 | 165.4 | live regions with controls are content, not announcements |
| 2026-11-01T17:31:39Z | 3e89d75+dirty | default | 51/54 | 12/12 | 3 | 228 | 228 | 0.0272 | 88.7 | 171.8 | after the review: secrets masking, Enter in a paying form, frame dialogs |
