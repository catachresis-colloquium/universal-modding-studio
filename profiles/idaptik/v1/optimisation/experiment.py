#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
"""Linux experiment driver only; production solver/validator are Rust.
No third-party packages. Seeded corpus bytes are the reproduction authority.
"""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]


def make(name, rows, minima=None, maxima=None):
    n = len(rows[0]) if rows else 2
    return {
        "format": "idaptik-guard-placement/v1", "id": name,
        "base_script": {"profile": "idaptik", "target": "exchange-house", "edits": [
            {"verb": "add_zone", "id": f"z{j:02}", "securityTier": 0,
             "worldXStart": j * 100, "worldXEnd": (j + 1) * 100} for j in range(n)]},
        "zones": [{"id": f"z{j:02}", "min": (minima or [0]*n)[j],
                   "max": (maxima or [len(rows)]*n)[j]} for j in range(n)],
        "guards": [{"id": f"g{i:02}", "rank": "BasicGuard",
                    "costs": {f"z{j:02}": c for j, c in enumerate(row) if c is not None}}
                   for i, row in enumerate(rows)],
    }


def corpus():
    cases = [
        make("ties", [[1, 1], [1, 1]], [1, 1], [1, 1]),
        make("swap", [[1, 2], [2, 100]], [1, 1], [1, 1]),
        make("three-cycle-gap", [[2, 3, 100], [100, 2, 2], [0, 100, 2]], [1]*3, [1]*3),
        # Equal domain sizes; greedy occupies z00 although the final guard needs it.
        make("greedy-dead-end", [[0, 1, None], [0, None, 1], [0, None, 1]], [1]*3, [1]*3),
        make("infeasible-quota", [[1, 2]], [1, 1], [1, 1]),
        make("infeasible-eligibility", [[1, None], [1, None]], [1, 1], [1, 1]),
        make("empty", []),
        make("zero", [[0, 0], [0, 0]], [0, 0], [2, 2]),
    ]
    # Explicit LCG, independent of Python random implementation/version.
    for seed in range(1, 13):
        state = seed
        n, m = [(4, 3), (6, 3), (8, 4), (9, 4)][(seed-1)//3]
        rows = []
        for i in range(n):
            row = []
            for j in range(m):
                state = (1664525 * state + 1013904223) & 0xffffffff
                row.append(None if state % 7 == 0 else (state >> 8) % 101)
            rows.append(row)
        cases.append(make(f"lcg-{seed:02}-{n}x{m}", rows, [1]*m, [(n+m-1)//m]*m))
    cases.append(make("large-32x8", [[(i*17+j*31+i*j*7)%101 for j in range(8)] for i in range(32)], [4]*8, [4]*8))
    return cases


def oracle(p):
    """Independent Cartesian-product oracle; no Rust search or counters."""
    guards = sorted(p["guards"], key=lambda g: g["id"])
    best = None
    for choices in itertools.product(*(sorted(g["costs"]) for g in guards)):
        if any(not z["min"] <= choices.count(z["id"]) <= z["max"] for z in p["zones"]):
            continue
        key = (sum(g["costs"][z] for g, z in zip(guards, choices)), choices)
        if best is None or key < best:
            best = key
    return best


def check(p, out):
    s = out["solution"]
    if s is None:
        assert out["edit_script"] is None
        return
    a = s["assignment"]
    assert set(a) == {g["id"] for g in p["guards"]}
    assert s["cost"] == sum(g["costs"][a[g["id"]]] for g in p["guards"])
    assert all(z["min"] <= list(a.values()).count(z["id"]) <= z["max"] for z in p["zones"])
    edits = p["base_script"]["edits"] + [
        {"verb": "add_guard", "id": g["id"], "rank": g["rank"], "zone": a[g["id"]]}
        for g in sorted(p["guards"], key=lambda g: g["id"])]
    assert out["edit_script"] == dict(p["base_script"], edits=edits)


def invoke(binary, solver, path):
    # Spawn/parse/replay/solve/validate/serialize/metrics/exit are all inside the wall
    # interval. Python JSON decoding and oracle checks are outside it.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        actions = [(os.POSIX_SPAWN_DUP2, stdout.fileno(), 1),
                   (os.POSIX_SPAWN_DUP2, stderr.fileno(), 2)]
        start = time.perf_counter_ns()
        pid = os.posix_spawn(str(binary), [str(binary), solver, str(path)], dict(os.environ, IDAPTIK_PLACEMENT_METRICS="1"), file_actions=actions)
        _, status, _ = os.wait4(pid, 0)
        elapsed = time.perf_counter_ns() - start
        stdout.seek(0); stderr.seek(0)
        if os.waitstatus_to_exitcode(status):
            raise RuntimeError(stderr.read().decode())
        raw = stdout.read()
        metrics = json.loads(stderr.read())
    return json.loads(raw), elapsed, metrics["peak_rss_kib"], hashlib.sha256(raw).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--generate", action="store_true")
    ap.add_argument("--repeats", type=int, default=7)
    ap.add_argument("--output", type=Path, default=HERE / "results.json")
    args = ap.parse_args()
    if args.repeats < 2:
        ap.error("at least two repeats required")
    directory = HERE / "corpus"
    if args.generate:
        directory.mkdir(exist_ok=True)
        for p in corpus():
            (directory / (p["id"] + ".json")).write_text(json.dumps(p, indent=2, sort_keys=True)+"\n")
    binary = ROOT / "target/release/idaptik-place-guards"
    if not binary.exists():
        ap.error("build first: cargo build --release -p ums-ai-edit --bin idaptik-place-guards")
    data = {"format": "idaptik-placement-experiment/v1", "environment": {
        "platform": platform.platform(), "cpu": next((s.split(":",1)[1].strip() for s in Path("/proc/cpuinfo").read_text().splitlines() if s.startswith("model name")), "unknown"),
        "logical_cpus": os.cpu_count(), "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "ums_base": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "sources_sha256": {f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in [HERE/"main.rs", HERE/"experiment.py"]},
        "memory": "Linux /proc/self/status VmHWM KiB at completion, per executable after exec; includes runtime, not allocator-only",
        "timing": "warm filesystem, fresh process per sample; one untimed warmup per solver/instance; alternating solver order; build and oracle excluded",
        "repeats": args.repeats}, "instances": []}
    for path in sorted(directory.glob("*.json")):
        p = json.loads(path.read_text())
        small = len(p["guards"]) <= 12
        truth = oracle(p) if small else None
        modes = ["exact", "heuristic"] if small else ["heuristic"]
        records = {mode: [] for mode in modes}
        outputs = {}
        for mode in modes:
            invoke(binary, mode, path)
        for repeat in range(args.repeats):
            for mode in (modes if repeat % 2 == 0 else list(reversed(modes))):
                out, ns, rss, digest = invoke(binary, mode, path)
                check(p, out)
                if mode == "exact":
                    observed = None if out["solution"] is None else (out["solution"]["cost"], tuple(out["solution"]["assignment"][g["id"]] for g in sorted(p["guards"], key=lambda g:g["id"])))
                    assert observed == truth, (path, observed, truth)
                if truth is not None and out["solution"] is not None:
                    assert out["solution"]["cost"] >= truth[0]
                records[mode].append({"wall_ns": ns, "peak_rss_kib": rss, "output_sha256": digest})
                outputs[mode] = out
        item = {"id": p["id"], "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "oracle": "exhaustive" if small else "not_run_large",
                "optimum": truth[0] if truth else None, "solvers": {}}
        for mode in modes:
            samples = records[mode]
            times = [s["wall_ns"] / 1e6 for s in samples]
            out = outputs[mode]
            cost = out["solution"]["cost"] if out["solution"] is not None else None
            item["solvers"][mode] = {"output":out, "samples":samples, "distinct_outputs":len({s["output_sha256"] for s in samples}),
                "absolute_gap":cost-truth[0] if truth is not None and cost is not None else None,
                "wall_ms": {"min":min(times), "median":statistics.median(times), "max":max(times), "stdev":statistics.stdev(times)},
                "peak_rss_kib": {"min":min(s["peak_rss_kib"] for s in samples), "max":max(s["peak_rss_kib"] for s in samples)}}
        data["instances"].append(item)
        print(p["id"], {m:(outputs[m]["status"], outputs[m]["solution"]["cost"] if outputs[m]["solution"] else None) for m in modes}, flush=True)
    args.output.write_text(json.dumps(data, indent=2, sort_keys=True)+"\n")


if __name__ == "__main__":
    main()
