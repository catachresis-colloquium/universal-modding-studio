<!--
SPDX-License-Identifier: CC-BY-SA-4.0
SPDX-FileCopyrightText: 2025-2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
-->

# Universal Modding Studio Component Readiness Assessment

**Standard:** [Component Readiness Grades (CRG) v2.0](https://github.com/hyperpolymath/standards/tree/main/component-readiness-grades)
**Assessed:** 2026-11-15, second status pass (WEEVIL-HOST-ROUTE package B3), on
**source commit `8901ef1`** (UMS `main`). See "Gate record, 2026-11-15 (second
pass)" below. The first pass ran on `a7031eb` (#112).
Rows not re-run on that day keep their own "Last assessed" date. Previous
assessments: 2026-07-25, 2026-07-22.
**Assessor:** Claude (PR E of the staged lineage migration), from real local runs
and the repo's CI workflows — evidence over intuition, no aspirational grading.

**Current Grade:** D

## Summary

| Component | Grade | Release stage | Evidence summary | Last assessed |
|---|---|---|---|---|
| Profile SDK (`crates/ums-profile-sdk`) | C | Alpha-stable | **2026-11-15 at `8901ef1`: 126 tests pass in the default suite** (8 unit, 34 `process_preview`, 32 `explain_golden`, 11 `profiles`, 20 `vendored_idaptik`, 11 `vendored_tom`, 9 `vendored_enaction`, 1 `cytoville`), plus 8 `#[ignore]`d `real_chain` tests that passed by hand (see the Enaction adapter row). Earlier record: 12 tests cover registration, malformed ID/version rejection, reflection, fixtures, deterministic adapter behavior, duplicate refusal, two-way profile isolation, IDApTIK compiler execution through `packageCompiler`, explicit Slavia no-compiler behaviour, unknown-profile refusal and rejection of an unsupported declared compiler version. The SDK hosts the CLI and is the sole repository-public profile surface; the implementation crate is non-publishable. A direct `cargo package -p ums-profile-sdk` probe currently rejects the internal path dependency, so standalone registry publication is not claimed. | 2026-11-15 |
| IDApTIK edit compatibility engine (`crates/ums-ai-edit`, Rust/Crusoe) | C | Alpha-stable | 65 tests (miniKanren kernel, verbs, six runtime relations, deterministic replay and explicit profile dispatch) + sample replay, gated by `rust-ci.yml` with `clippy -D warnings`. | 2026-08-29 |
| Package/DLC bridge (`crates/ums-dlc` + `schemas/`) | C | Alpha-stable | 39 tests; validates all in-tree artifacts, legacy-manifest compatibility, v1→v2 migration and capability declarations. | 2026-07-25 |
| Chronicles of Slavia profile | D | Design fixture | Reflection and isolation are tested against a minimal Zone A fixture; no UMS compiler, loader or runtime integration exists. | 2026-07-25 |
| Enaction adapter (`ProcessPreviewAdapter`, ADR-0019 Accepted) | D (was X) | Real chain runs in CI from 2026-11-19 (first run green: `real-chain.yml` run `44427809761`, 8/8); not a required status check, so a red run does not block merge | The adapter runs a pre-built runner twice and a validator once as processes, with replay identity enforced on every preview. 34 stub-protocol tests and 32 explain-lens tests run in the default suite. On 2026-11-15 at `8901ef1`, all 8 `#[ignore]`d `real_chain` tests passed against `enaction-validate` (enaction-engine `724e502`) and `idaptik-weevil-replay` (IDApTIK `802b009`, kernel evil-weevil `bffcbe3`). So did `just preview-weevil`, which reports 37 commitment-override ticks for the grudge. From 2026-11-19 `.github/workflows/real-chain.yml` builds the three binaries in CI and runs these tests: pinned sibling commits on pull requests and pushes, sibling heads weekly. The grade change from X to D was accepted by the owner on 2026-11-19, with ADR-0019. | 2026-11-15 |
| Generation source of truth (`config/*.ncl`) | C | Alpha-stable | 2026-11-15 at `8901ef1`: `just config-check` and `just gen-check` pass, and the tree is unchanged after generation, with Nickel **1.18.0**. `mise.toml` pins 1.16.0; the pinned version was not run. Earlier record: `config-check` typechecks every source AND requires all three `config/bad/bad_*.ncl` negative fixtures to be rejected; `gen-check` diffs generated artifacts and fails when `nickel` is absent rather than skipping. Gated by `config-gen.yml`. | 2026-11-15 |
| Zig FFI (`ffi/zig/`) | C | Compatibility FFI | **2026-11-15 at `8901ef1`, Zig 0.14.0: 32 of 32 integration tests pass.** The one failure recorded at `a7031eb` was fixed by #114. `ace5777` had renamed the guard rank `chief` to `security_chief` in admission, but not in `tests/abi-roundtrip/full-c-layout.json` or in the JSON codec. The compiled-symbol gate passes (85 exports match the manifest). The recipes were run as their underlying `zig` commands, because `mise` is absent. **Not CI-gated in practice:** every `zig-ci.yml` run since 2026-11-01 ends in `startup_failure` (runs 44 to 48, the last on `main` at #114). The likely cause is that the organisation's Actions policy does not allow `mlugg/setup-zig`; this is an owner decision. Earlier record: 32 integration tests are CI-gated and Zig 0.14.0 is pinned. The exported full-JSON admission endpoint agrees with Idris2 on six shared fixtures. The complete bounded C codec preserves active fields across a rich C→JSON→C→JSON test with byte-stable second serialization. The generated Idris2 ABI artifacts cover all 85 independently enumerated C exports, 27 structs, 27 enum representations and 142 discriminants. Tests compare all layouts, discriminants and function ABI shapes; the compiled shared/static symbol gate consumes the generated 85-symbol manifest. Planted layout, signature and symbol drift each fail. These are executable tests, not universal parser, semantic-type or cross-platform layout proofs. Unified-hexadeca integration remains unstarted because no canonical implementation is locatable. | 2026-11-15 |
| Licence hygiene gate | C | Alpha-stable | Three steps, each negative-tested: a planted MPL header, a truncated LICENSE and an unattributed JSON file each make it fail. Polarity inverted with the AGPL relicence. | 2026-07-22 |
| Idris2 ABI (`abi/`) | C | Beta | All 19 modules typecheck, including `Representation`, `ProvenBridge` and the total `CABI` declaration renderer. Raw JSON extraction is private; the exported parser first checks bounded C-value representability, then admits only `ValidatedLevel` values after all four `Dec` procedures construct their witnesses. The extractor executable reports 51/51 checks and the separate representation executable reports 17/17. Successful refinement carries source-preservation equality; canonical optional payloads and all `ItemKind` variants have dependent re-encoding equalities. The renderer generates the complete current 85-function Zig C-export header, compatibility include and symbol manifest, all byte-drift checked. Complete semantic-record-to-C-struct conversion, auxiliary session/player refinement, universal parser equivalence and cross-platform ABI equivalence remain unproved. No `believe_me`, `postulate`, `assert_total`, `assert_smaller`, `idris_crash` or `unsafePerformIO`, and every package module declares `%default total`: `scripts/check-abi-trusted-base.sh` gates both in `idris-ci.yml` (and `just proof-check-abi`), and fails on any `abi/` file that no package declares. Negative-tested with a planted `believe_me`, an undeclared module, a commented-out `%default total`, an undeclared harness and a 20th module, and since PR #97's review with a qualified module (`Extra.Helper`, resolved to `abi/Extra/Helper.idr`), an undeclared file in a subdirectory, a `%default total` placed after the first declaration, `%default covering`, and `partial` and `covering` modifiers. Caveat: CI builds against a `proven` with `Proven.SafeMath.Proofs` removed, disclosed in `scripts/proven-min.ipkg`. | 2026-08-30 |
| SPARK/GNATprove reference model (`spark/`) | C | Alpha-stable | **Verification conditions discharged locally**: `gnatprove --level=2` (FSF 16.1.0) reports no unproved checks: 19/19 run-time checks, 8/8 assertions, 7/7 termination, and the `Zones_Ordered` postcondition proved. The postcondition states the quadratic specification; the body is the linear sweep. Not linked at runtime (ADR-0003 §3). `just spark-parity` drives 26 shared vectors through the SPARK model and the Rust engine with identical verdicts. Negative-tested: a body that diverges from its postcondition, an empty vector table, a missing prover, and a prover run that proves nothing each fail; the earlier symmetric specification fails to prove. `spark-ci.yml` runs the same proof and parity gates in CI whenever `spark/`, the Rust engine or the gate scripts change. | 2026-10-09 |
| Zig hexadeca connector | X | Blocked dependency | Does not exist. Estate reconnaissance cannot locate a canonical `unified-hexadeca-api`; Hypatia's project-local 16-transport pattern is not silently copied here. | 2026-08-29 |
| Interactive studio frontend | D | Local alpha, runtime gate pending | 2026-11-15 at `8901ef1` on Linux with Bun 1.4.4: frozen install, 13 Bun tests, typecheck and production build pass. The language policy question is still open (see `docs/EXPLAINME.adoc`): the studio is TypeScript and no exemption is recorded. Earlier record: Vue 3/Bun UI now includes a profile-reflected edit composer, closed vocabulary controls and Rust CLI validation bridge. Bun 1.4.4 frozen install, 13 Bun tests, strict TypeScript and Vite production build passed on a Windows development checkout on 2026-11-01; the local API bootstrap served both descriptors. The local Bun HTTP API smoke test succeeded: descriptor bootstrap, accepted IDApTIK edit replay, refusal of invalid vocabulary and unimplemented Slavia edits, Slavia fixture validation, and IDApTIK package compilation against a fresh game-owned contract checkout. The full IDApTIK game loader was not executed through the UI. The organisation's Studio workflow still fails at startup without running jobs. Most canvas lenses remain unimplemented. | 2026-11-15 |
| Reversible VM (`dlc/vm/`, `.affine`) | X | — | Has never compiled. No AffineScript toolchain is wired to this repo; the `.affine` sources have never been exercised by anything, so its declared `every-instruction-has-an-inverse` guarantee has never been checked. | 2026-07-22 |

## Gate record, 2026-11-15 (second pass)

Source commit **S = `8901ef1`** (UMS `main`, #117 "Re-vendor the corridor from
IDApTIK 802b009"). Every gate below ran on S with an unchanged working tree,
except where marked OPEN. Sibling checkouts were read-only clones at the heads
of their default branches: enaction-engine `724e502`, evil-weevil `bffcbe3`
(its own `zig build test`: 53/53), IDApTIK `802b009`, chronicles-of-slavia
`21b1661`. The first pass of the same day ran on `a7031eb` (#112).

| Gate | Result | Evidence |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | exit 0 |
| `cargo clippy --locked --all-targets -- -D warnings` | PASS | exit 0 |
| `IDAPTIK_ROOT=… cargo test --locked` | PASS | 245 passed, 0 failed, 9 ignored |
| `real_chain` (8 `#[ignore]`d tests, real binaries) | PASS | 8 of 8 |
| `just preview-weevil` (the grudge) | PASS | 37 commitment-override ticks; `replay-identical: ok` |
| `just roundtrip-idaptik` | PASS | accepted, executed, snapshotted, restored, replayed identically |
| CI round-trip step | PASS | Rust CI run `43703053968` on `main` at S, job `132778148858`, step 10 "UMS to IDApTIK round trip through the real game loader" |
| `just architecture-check` (all three `*_ROOT`) | PASS | dependency direction and profile isolation verified |
| `bash tests/boundaries-planted-violation.sh` | PASS | 11 planted violations fail the gate; the clean estate passes |
| `just dlc-check` | PASS | 31 artifacts checked, 0 invalid |
| `just slavia-profile-check` | PASS | Border Path fixture valid |
| `just ai-edit-check` | PASS | exit 0 |
| `just config-check`, `just gen-check` | PASS | Nickel 1.18.0, not the pinned 1.16.0; no drift |
| Studio: `bun install --frozen-lockfile`, `bun test`, `bun run build` | PASS | 13 tests; typecheck and build pass |
| Zig FFI integration tests (`zig build test`, Zig 0.14.0) | PASS | 32 of 32 (31 of 32 at `a7031eb`; fixed by #114) |
| `zig fmt --check src test` | PASS | exit 0 |
| Zig compiled-symbol gate (`zig build` + `scripts/check-abi-symbols.sh`) | PASS | 85 exports match the manifest |
| Licence hygiene workflow | PASS | green on #112 (run `43656374387`) and on #114 (run `43699275246`), both after #109 |
| Zig CI workflow (`zig-ci.yml`) | **FAIL** (pre-existing) | `startup_failure` on every run since 2026-11-01 (runs 44 to 48); likely the organisation's Actions policy refuses `mlugg/setup-zig`; owner decision |
| `just test-ffi`, `just abi-symbol-check`, `just fmt-check` as recipes | OPEN | `mise` absent; the guard refuses. The same `zig` commands ran directly (rows above) |
| `just proof-check-abi`, `just abi-header-check` | OPEN | Idris2 absent |
| `just proof-check-spark` | OPEN | GNATprove absent |
| REUSE lint with the `reuse` tool | OPEN | `reuse` absent. The licence workflow checks attribution without that tool |
| `dlc/vm` reversibility | OPEN | AffineScript toolchain absent; the VM has never compiled |

## Why Grade D — for a much narrower reason than before

CRG defines the project line as the grade of the worst deployed component. It
is still **D**, but the reason has changed completely.

The 2026-07-20 assessment was held at D by three things: the engine was Python
with no CI, the DLC schema check ran local-only, and the ABI was 16/17. **All
three are now resolved.** The engine and the validator are Rust/Crusoe, CI-gated, with
profile, engine and package negative tests demonstrating the gates can fail.

`abi/ProvenBridge.idr` landed on 2026-07-20 (`c86c84c`) and its extractors on
2026-07-21 (`a8b7663`). It has no typed holes, `idaptik-ums.ipkg` declares
`depends = proven` uncommented, and `idris-ci.yml` has been green on every run
since 2026-07-22. **The D grade was held for five days by this table, not by
the tree** — the row was never re-run after the work landed. Re-assessed
2026-08-30 against a local reproduction of the CI pipeline: 19/19 modules
typecheck, extractor/admission tests 51/51 and representation controls 17/17.

What remains:

- **The Idris extraction boundary is closed; bounded cross-language admission
  parity is tested.** Total `Dec`-returning procedures construct witnesses for the four
  `ValidatedLevel` obligations. `ProvenBridge` keeps raw extraction private
  and exports only `parseValidatedLevelJson`; planted semantic negatives cover
  every witness class. The exported Zig endpoint agrees on the shared six-case
  corpus. The bounded C codec also has a complete active-field round-trip test.
  The generated header additionally covers all current Zig C exports with
  executable layout, discriminant, signature-shape and compiled-symbol parity.
  None of these results proves universal parser equivalence or cross-platform
  raw layout equality.
- **Bounded semantic admission is machine checked, while the complete physical
  conversion remains open.** `Representation.refineLevel` checks capacities,
  `uint32_t` narrowing and C-string sentinels and carries equality showing that
  successful admission preserves its `LevelData` source. Optional flag/payload
  pairs and every `ItemKind` variant have canonical dependent decoders. The
  full Zig array copy, pointer lifetime and auxiliary session/player semantic
  refinement remain tested or unmodelled rather than proved.
- **Not X or E:** every component above the D-line runs real, failing-able
  tests that currently pass, with documented scope.
- The X-graded components (frontends, hexadeca connector, VM) are
  not deployed and gate nothing — but they are why this cannot claim more than
  alpha-unstable, because the studio still has no interactive surface and the
  VM has never compiled.

**Assessment basis.** The Rust and Nickel gates were verified by local runs of
the exact commands CI executes. `rust-ci.yml` and `config-gen.yml` are new
files whose first CI execution happens when their branches reach `main`; every
workflow here filters on `pull_request: branches: [main]`, so a stacked PR
does not trigger them. Graded on measured local evidence, with that caveat
stated rather than papered over.

## Known failures and debt (kept visible on purpose)

- `governance.yml` is red on `main` — pre-existing, estate-wide (thin wrapper
  over `hyperpolymath/standards` reusables that fail at startup). Not
  repo-local; fix is upstream.
- `push-email-notify.yml` is the estate-wide never-green template.
- ~~Python is banned by the estate language policy; `ai_edit` landed as
  Python~~ — **resolved 2026-07-22.** `ai_edit/`, `scripts/validate_dlc.py`
  and both test modules are deleted. ADR-0001 is superseded by ADR-0003.
  *Correction, 2026-11-14:* `git ls-files '*.py'` is not empty. It lists
  `profiles/idaptik/v1/optimisation/experiment.py`, and no exemption is
  recorded (see `docs/EXPLAINME.adoc`).
- `abi/Types.idr` and `ffi/zig/src/types.zig` are still hand-written rather
  than generated from `config/vocab.ncl` — the last two of the original six
  copies. They are now **gated** (`crates/ums-dlc/tests/vocab_parity.rs`, 5
  tests): the four shared vocabularies must match the Nickel source in
  declaration ORDER, and the Zig `enum(u8)` ordinals must be dense from zero,
  because those bytes cross the FFI boundary and a reordering that keeps the
  same members would silently remap every value. Generating them outright is
  still the better end state; wholesale generation is blocked because both
  files also carry ABI-only types (`AlertLevel`, `ItemCondition`, the
  cable/adapter/tool enums) that have no place in an edit vocabulary.
- The UMS -> game round trip is CI-gated again. It passed in CI on `main` at
  `8901ef1` (Rust CI run `43703053968`, step 10), and locally against IDApTIK
  `802b009`. From the loss of IDApTIK's `package-runner` example until its
  restoration at `58d6936` it could not pass (see
  `docs/ROUNDTRIP-STATUS.adoc`).
- Zig CI has ended in `startup_failure` on every run since 2026-11-01, so the
  Zig FFI suite is not CI-gated in practice. The suite itself passes 32 of 32
  locally since #114; see the Zig FFI row.
- Until 2026-07-20 the Justfile's `test-all` chained five echo-stubs and
  printed "safe to merge!". Those recipes are deleted; every remaining gate
  runs real work and can fail.

## Promotion paths

- **PROJECT D → C: done, 2026-07-27.** `ProvenBridge` landed on 2026-07-20 and
  the tree and the module-count claim now agree. No descope ADR is needed:
  descoping something that works would be the wrong record.
- **PROJECT C -> B:** the Idris extraction boundary consumes a
  `ValidatedLevel`, bounded JSON admission parity is tested, and the typed
  Idris header renderer plus drift/layout gates are implemented for the bounded
  `LevelData` surface. Remaining work includes explicit refinement evidence
  from semantic Idris records to the fixed C model and integration with a
  canonical unified-hexadeca implementation
  once one exists. The shared fixtures and executable layout checks are not
  cross-language proofs.
- **ai-edit C → B:** grow a real consumer, and close the type-6 loop so the
  proposer consults `solve()` in-process rather than across a boundary.
- **DLC bridge C -> B:** keep the implemented cross-repository round trip green
  across versioned game contract changes and add an explicit negative drift
  fixture.
- **SPARK model C → B:** close the fidelity gap: `worldX` may be fractional
  while `World_Coord` is an integer range.
- **Frontends X → D:** first execute and test the new Vue/Bun alpha with the real sibling game contracts, then implement an independently testable authoring interface; do not promote grade on an unrun CI workflow.
  mirroring `idaptik-tui`.
- **VM X → D:** port `dlc/vm` to Rust so it compiles at all, and make its
  `every-instruction-has-an-inverse` guarantee a round-trip property test over
  all 23 instructions, before claiming anything about reversibility.

## Machine-readable

The grade line near the top of this file (`Current Grade`) is parsed by
`just crg-grade` and `just crg-badge`; before this file existed both recipes
silently fell back to grade "X".
