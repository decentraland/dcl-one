## Benchmark of #12

PR #12 · change: `abc1234` · base: `main` at `def5678` · [run](https://github.com/example/repo/actions/runs/7)

**Overall: 🟢 CPU average improved on Apple M1 (Chrome), Intel Core i5 (Chrome)**

<details><summary>How to read this table</summary>

- Each build is measured **3 times**, interleaved with the other build (change, baseline, change, baseline, ...) in the same session, so both see the same world content and machine state. The values are the **median**, and **(min–max)** is the lowest and highest of those runs.
- **Δ** is Change minus Baseline (a negative Δ means Change is faster).
- **🟢 faster / 🔴 slower** — a difference that passed every check: the runs are fully separated (every run of one build faster than every run of the other), and the median difference is at least **3%** and at least **0.5 ms**.
- **⚪ within noise** — the builds' runs overlap, or the difference is tiny; it cannot be told apart from random variation. Treat it as no change.
- **— informational** — the 0.1% worst metrics average only the few worst frames of a run, so a single OS hiccup swings them by a lot; they are shown for context and never earn a verdict.
- **⚠️ no verdict** — the two builds' sessions were not comparable (very different sample counts, or too few usable runs), so no conclusion is drawn from them.
- **Exceptions per run** — the average number of exceptions in a run's log, not counting teardown ones logged while the app quits. Flagged only on a difference of at least 2 per run and 1.5× the other build; exception kinds the baseline never threw are called out under the table. The *Exception breakdown* groups all of them by the explorer's report category and exception type (as totals across the runs).
- A run that logged **unusually many exceptions** (at least 10 and 5× the median of its build's runs — e.g. a service was down during it) is excluded from all numbers and called out under the table.
- The **Overall** line at the top only reacts to a metric that moved on **two or more machines**, or by **10% or more** on one — a single modest 🟢/🔴 cell can still be a statistical fluke.
</details>

### Apple M1 (Chrome)

| Metric | Baseline | Change | Δ | Result |
| --- | ---: | ---: | ---: | :--- |
| Samples | 10 (×3) | 10 (×3) | | |
| CPU average | 23.0 ms (22.9–23.1) | 21.0 ms (20.9–21.1) | -2.0 ms | 🟢 9% faster |
| CPU 1% worst | 32.0 ms (31.4–32.6) | 30.0 ms (29.0–31.0) | -2.0 ms | 🟢 6% faster |
| CPU 0.1% worst | 32.0 ms (31.4–32.6) | 30.0 ms (29.0–31.0) | -2.0 ms | — informational |
| GPU average | 6.4 ms (6.3–6.5) | 6.4 ms (6.3–6.5) | 0.0 ms | ⚪ within noise |
| GPU 1% worst | 10.0 ms (9.0–11.0) | 10.0 ms (9.0–11.0) | 0.0 ms | ⚪ within noise |
| GPU 0.1% worst | 10.0 ms (9.0–11.0) | 10.0 ms (9.0–11.0) | 0.0 ms | — informational |
| Exceptions per run | 0 | 0 | 0 | ⚪ no significant change |

### Intel Core i5 (Chrome)

| Metric | Baseline | Change | Δ | Result |
| --- | ---: | ---: | ---: | :--- |
| Samples | 10 (×3) | 10 (×3) | | |
| CPU average | 34.0 ms (33.8–34.4) | 31.0 ms (30.8–31.2) | -3.0 ms | 🟢 9% faster |
| CPU 1% worst | 43.0 ms (41.0–47.0) | 40.0 ms (38.0–42.0) | -3.0 ms | ⚪ within noise |
| CPU 0.1% worst | 43.0 ms (41.0–47.0) | 40.0 ms (38.0–42.0) | -3.0 ms | — informational |
| GPU average | 8.3 ms (8.2–8.5) | 8.2 ms (8.1–8.4) | -0.1 ms | ⚪ within noise |
| GPU 1% worst | 11.0 ms (10.0–13.0) | 10.0 ms (9.0–12.0) | -1.0 ms | ⚪ within noise |
| GPU 0.1% worst | 11.0 ms (10.0–13.0) | 10.0 ms (9.0–12.0) | -1.0 ms | — informational |
| Exceptions per run | 1 | 1 | 0 | ⚪ no significant change |

<details><summary>Exception breakdown</summary>

| Exception | Baseline (3 runs) | Change (3 runs) |
| --- | ---: | ---: |
| `[comms] Error` | 3 | 3 |
</details>

