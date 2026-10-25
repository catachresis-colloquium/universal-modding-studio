<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->
<!-- SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk> -->

# Offline IDApTIK guard placement

This authoring tool assigns new guards to existing zones and emits a replayable
IDApTIK edit script. It is implemented and tested in this profile directory,
hosted as a binary of the existing `ums-ai-edit` compatibility facade. No new
crate, universal problem vocabulary, game dependency or quantum dependency.

## Contract decision

Inspected UMS `a457e47fe2fe94ebcb20401a834cddc8eb67c957`, its AGENTS.md,
miniKanren kernel, six constraints, generated `add_guard` registry, profile
SDK/compiler, ADRs 0003, 0008, 0014, 0016–0018, and game-owned
`contracts/idaptik/v1/{contract.json,package.schema.json,fixtures/ghost-lobby-scenario.json}`
at IDApTIK `ae6125e4715edbf174e40461b66323437e1c70b7`.

The published game package has a fixed Ghost Lobby scenario and actor-role
bindings, not a multi-guard patrol assignment contract. Billy's patrol tuning
is not a multi-agent routing API. Consequently this slice optimises **guard
placement**, using the existing `add_guard(id, rank, zone)` authoring verb.
It does not invent patrol routes, extend the game package, modify the game-owned
scenario, or claim that these placements can currently be shipped to the game.
The next required integration is a game-owned placement/package contract.

The existing miniKanren facilities enumerate finite-domain edits and validate
six cross-domain constraints; they do not provide a weighted assignment
objective. Dedicated bounded search stays here; every emitted solution still
passes the real profile edit dispatcher and miniKanren replay. No existing
constraint is bypassed or weakened. These are runtime-validated/tested claims,
not machine-checked optimality proofs.

## Problem and objective

Input format `idaptik-guard-placement/v1` is an **authoring request**, not a
replacement game schema. See `corpus/` for complete machine-readable examples.
Authors supply:

- A valid `base_script` for the `idaptik` profile, with unique declared zones
  and no pre-existing guards. All its other authored content is preserved.
- Zone lower/upper guard quotas `min`, `max` (0 through 64).
- Unique guard ids, actual registry ranks, and integer `costs` by eligible zone.
  Missing pairs are forbidden. Costs are author preferences/penalties, not
  inferred traversal times or simulated gameplay quality.

For binary choices x(g,z), minimise sum(cost(g,z) * x(g,z)), subject to exactly
one eligible zone per guard and min(z) <= sum_g x(g,z) <= max(z). No time slots,
paths, collision avoidance, per-rank coverage or movement costs are modelled.
Cost accumulation uses u64; 64 u32 costs cannot overflow it. Unknown fields in
problem/guard/zone/solution records, duplicate ids, invalid ranks, undeclared
zones and invalid base scripts are rejected. Empty guard sets are allowed;
positive lower quotas then make the instance infeasible.

Canonical order is ascending Rust string order of guard ids and zone ids.
Among equal-cost candidates choose the lexicographically smallest zone vector
in guard order. Input array/map order has no effect. Exact search chooses this
tie globally. The heuristic uses it for choices within its neighbourhood and
does not promise the globally canonical optimum.

## Algorithms and validation

`exact`: depth-first enumeration of eligible vectors, pruning exceeded upper
quotas. Check lower quotas at leaves. Refuse more than 12 guards or an eligible
Cartesian product above 2,000,000, rather than silently truncating search.
No valid leaf is pruned by an objective estimate. An exhausted search can
report `infeasible`. General input bound: 64 guards and 1–16 zones.

`heuristic`: process guards by fewest eligible zones, then guard id; prefer
zones below their minimum, then cost, then zone id. On a feasible initial
assignment, examine every legal one-guard move and two-guard swap, selecting
the best cost/canonical improvement. Stop at no improvement or 128 iterations.
It can get stuck at a non-global optimum or fail to construct a feasible
assignment. Such failure is `no_candidate`, **not** `infeasible`. There is no
random seed in either solver; seed variability is absent by design, while
instance variability and timing variability are measured.

The independent complete-candidate validator does not trust solver counters:
it checks assignment cardinality, identities, eligibility and quotas,
recomputes cost, rebuilds the emitted script, and replays it through
`apply_profile_edit_script("idaptik", ...)`. It runs for every returned solution
and is separately accessible using `validate INSTANCE SOLUTION`. The solution
file is the `solution` object from solver output. This validates feasibility
and reported cost, **not** optimality. An independent Python Cartesian-product
oracle checks all small corpus optima including canonical ties. Rust tests also
compare 256 binary cost matrices against a separately structured oracle and
exercise corrupt candidates and invalid inputs.

This linear capacitated assignment problem admits a polynomial min-cost-flow
formulation. The enumerator is a bounded reference, not a state-of-the-art
classical solver. A flow solver is the sensible next scaling comparison; these
results provide no argument for quantum advantage.

## Commands

Run from repository root (Rust 1.95.0; Python 3 standard library; Linux procfs
for memory measurements):

```sh
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p ums-ai-edit --bin idaptik-place-guards
target/release/idaptik-place-guards exact profiles/idaptik/v1/optimisation/corpus/swap.json > /tmp/placement.json
jq '.edit_script' /tmp/placement.json > /tmp/placement-edits.json
cargo run -q -p ums-ai-edit -- check /tmp/placement-edits.json
jq '.solution' /tmp/placement.json > /tmp/placement-solution.json
target/release/idaptik-place-guards validate profiles/idaptik/v1/optimisation/corpus/swap.json /tmp/placement-solution.json
python3 profiles/idaptik/v1/optimisation/experiment.py --repeats 7 --output /tmp/placement-results.json
# Regenerate the same corpus using the explicit LCG plus adversarial examples:
python3 profiles/idaptik/v1/optimisation/experiment.py --generate --output /tmp/placement-results.json
# Convenience recipe (overwrites the checked-in results with your machine's run):
just idaptik-placement-experiment
just gen gen-check config-check ai-edit-reflect
just dlc-check ai-edit-check slavia-profile-check
just roundtrip-idaptik ../IDApTIK
```

The experiment launches a fresh process per observation, one untimed warmup
per solver/instance, then seven repetitions with alternating solver order.
Wall time includes launch, input read/parse, base validation, rank probes,
search, independent validation, edit serialization, metrics and exit. Build,
Python oracle and result analysis are excluded. No solver-only microbenchmark
is presented as end-to-end time. Memory uses the executable's Linux VmHWM,
read at completion; it includes Rust/runtime allocations and mapped pages,
not just search storage. `wait4.ru_maxrss` can retain the Python launcher's
high-water mark across exec, so it is deliberately not used as solver memory.

`results.json` contains input/source/binary hashes, environment, all raw samples,
full solutions/scripts, costs, gaps, output variability, timing summaries and
memory ranges. `RESULTS.md` interprets the measured run. The generated corpus
contains synthetic authoring requests with real accepted verbs, not observed
player telemetry or a representative benchmark of shipped levels.

## Future execution modes (not implemented)

| Mode | Meaning | Required evidence before comparison |
|---|---|---|
| Classical simulated annealing | CPU stochastic search using temperature schedules | Named seed/schedule, repeated quality distribution, same validator and full costs |
| Quantum simulation | Classical simulation of a specified quantum model | Encoding/parity tests, simulator resource costs, no hardware-speed claim |
| Actual quantum hardware | Execution on an identified physical device | Device/provider/version, embedding, queue/network/pre/postprocessing and repeated validated results |

These are distinct future modes. No provider, simulator, annealer, backend
placeholder or empty crate is introduced. Simulation is not hardware evidence;
simulated annealing is not quantum advantage. Enaction and shipped games retain
no UMS dependency, including dev/feature-gated dependencies.

## Existing game-package round-trip evidence

The normal `just roundtrip-idaptik ../IDApTIK` recipe fails at the game invocation:
IDApTIK at the inspected SHA has no `package-runner` example. Its four package
unit tests pass. A temporary external runner calling the real public
`idaptik_core::{load_package,run_package}` APIs accepted the **unchanged Ghost
Lobby package**, met guarantees, snapshotted/restored/replayed identically,
and passed `compare-authored-vs-accepted.sh` (all eight fields, nine taxonomy
terms, two actors and two commands). Repeated compilation was byte-identical.
This preserves evidence for the existing package path; it does not make the
new guard-placement edits game-loadable. Neither repository's game contracts
nor runtime dependency graph was changed.

To reproduce the fallback without adding a runner or dependency to a game:

```sh
probe=$(mktemp -d)
mkdir "$probe/src"
game_root=$(cd ../IDApTIK && pwd)
cat > "$probe/Cargo.toml" <<EOF_MANIFEST
[package]
name = "ums-package-probe"
version = "0.0.0"
edition = "2024"
[dependencies]
idaptik-core = { path = "$game_root/crates/idaptik-core" }
serde_json = { version = "1", features = ["float_roundtrip"] }
EOF_MANIFEST
cat > "$probe/src/main.rs" <<'EOF_RUST'
fn main() {
    let input = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let loaded = idaptik_core::load_package(&input).unwrap();
    let result = idaptik_core::run_package(loaded).unwrap();
    println!("{}", serde_json::to_string(&result).unwrap());
}
EOF_RUST
cargo run -q -p ums-profile-sdk --bin ums-profile -- compile-idaptik \
  --source profiles/idaptik/v1/ghost-lobby.ums.json --idaptik-root "$game_root" \
  --output "$probe/package.json"
cargo run -q --manifest-path "$probe/Cargo.toml" -- "$probe/package.json" > "$probe/result.json"
jq -e '.replay_equal and .package_guarantees_met and
  .snapshot.format == "idaptik-ghost-lobby-runtime-v3" and
  ([.events[].event] | index("CameraPinged") != null) and
  ([.cognitive_trace[].stage] | unique | length == 6)' "$probe/result.json"
scripts/compare-authored-vs-accepted.sh profiles/idaptik/v1/ghost-lobby.ums.json "$probe/result.json"
```

The native runner has only a game dependency; the game has no UMS dependency.
The temporary runner's crates resolve at reproduction time, so record its
Cargo.lock as well if comparing different game revisions.
