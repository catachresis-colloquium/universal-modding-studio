<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->
<!-- SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk> -->

# Measured placement experiment — 25 October 2026

21 reproducible instances; seven timed fresh-process runs per solver/instance.
20 small instances checked against an independent exhaustive Python oracle;
one 32-guard/8-zone heuristic-only case. 287 timed executions, plus warmups.
All returned assignments passed both Rust validation/replay and Python checks.
Exact matched every small oracle result and tie. Of 18 feasible small instances,
the heuristic found 17: 13 had optimal cost, four had excess costs of 1, 4, 3
and 1. It missed the deliberately adversarial `greedy-dead-end`. Both genuinely
infeasible inputs yielded `no_candidate` from the heuristic, not a false proof.

All 41 solver/instance combinations produced one unique output across seven
repetitions. No seed variability is claimed: these algorithms are deterministic.

Environment: AMD EPYC 9V74, 9 visible logical CPUs, Linux x86_64/glibc 2.39,
Rust 1.95.0, Cargo release defaults. Shared container with concurrent compilation
and no CPU pinning or isolation. Scheduling noise is material; these observations
are not a controlled speedup claim or a cross-platform forecast. The raw JSON
records min/max/stdev and hashes, not just the favourable medians.

| Instance | Optimum | Heuristic cost | Exact median ms | Heuristic median ms | Exact peak KiB range | Heuristic peak KiB range |
|---|---:|---:|---:|---:|---:|---:|
| empty | 0 | 0 | 7.786 | 6.079 | 1848–1912 | 1860–1908 |
| greedy-dead-end | 2 | no candidate | 8.534 | 5.622 | 1896–1944 | 1880–1940 |
| infeasible-eligibility | — | no candidate | 7.824 | 9.122 | 1872–1924 | 1868–1904 |
| infeasible-quota | — | no candidate | 7.909 | 5.337 | 1872–1900 | 1872–1924 |
| large-32x8 | — | 318 | — | 241.787 | — | 2340–2388 |
| lcg-01-4x3 | 114 | 114 | 20.671 | 7.640 | 1904–1952 | 1912–1960 |
| lcg-02-4x3 | 198 | 198 | 15.441 | 9.403 | 1896–1936 | 1916–1960 |
| lcg-03-4x3 | 113 | 113 | 15.658 | 8.475 | 1896–1960 | 1900–1948 |
| lcg-04-6x3 | 122 | 122 | 17.586 | 9.667 | 1932–1976 | 1924–1976 |
| lcg-05-6x3 | 170 | 170 | 24.320 | 20.132 | 1924–1956 | 1924–1972 |
| lcg-06-6x3 | 159 | 159 | 25.221 | 18.056 | 1916–1980 | 1924–1976 |
| lcg-07-8x4 | 236 | 236 | 20.879 | 26.607 | 1952–1988 | 1952–1984 |
| lcg-08-8x4 | 191 | 192 | 19.626 | 23.551 | 1948–2004 | 1988–2016 |
| lcg-09-8x4 | 270 | 274 | 30.032 | 32.919 | 1956–2004 | 1960–2008 |
| lcg-10-9x4 | 139 | 142 | 108.424 | 24.407 | 1972–2012 | 1968–2020 |
| lcg-11-9x4 | 194 | 194 | 75.716 | 23.229 | 1964–2020 | 1968–2024 |
| lcg-12-9x4 | 184 | 184 | 149.122 | 33.264 | 1964–2024 | 1972–2000 |
| swap | 4 | 4 | 4.410 | 3.759 | 1880–1932 | 1872–1908 |
| three-cycle-gap | 5 | 6 | 7.159 | 7.753 | 1892–1940 | 1884–1940 |
| ties | 2 | 2 | 3.856 | 3.797 | 1884–1924 | 1872–1928 |
| zero | 0 | 0 | 2.543 | 4.398 | 1880–1908 | 1892–1928 |

“—” means infeasible or not measured, as distinguished by `oracle` and solver
status in the JSON. The large case cost is 318; its optimum/gap is unknown.
Heuristic quality is not uniformly optimal, and lower latency is not uniform
across small cases. The three 9×4 cases show lower heuristic median time in this
run; the 32×8 case demonstrates a feasible bounded authoring output, not superiority
over a scalable exact assignment algorithm.

Peak resident memory is measured after exec using Linux VmHWM, including the
runtime, parser, validation and serializer. It is not live heap size, cumulative
allocation, or isolated search working memory. Tiny RSS differences include
mapping/page effects; no meaningful memory superiority is established.

See [README.md](README.md) for mathematical scope, commands, independent checks,
contract limitations and distinct future simulated-annealing, quantum-simulation
and actual-hardware modes. See [verification.json](verification.json) for check
evidence and blockers. No simulated or hypothetical benchmark numbers are used.
