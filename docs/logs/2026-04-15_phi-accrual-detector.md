# 2026-04-15 — Add φ-Accrual Detector (SOTA Baseline)

## What changed

Implemented the φ Accrual Failure Detector (Hayashibara, Defago, Yared, Katayama, SRDS 2004) as a fourth detection strategy and wired it into the configuration system end to end.

### Files added
- `src/detector/phi_accrual.rs` — `PhiAccrualDetector` plus a self-contained Abramowitz–Stegun erf approximation (no new dependencies). Includes 6 unit tests covering: cold start, low-φ on fresh heartbeats, high-φ on long silence, monotonic increase with delay, sliding-window eviction, and erf accuracy spot-checks.
- `configs/scenarios/crash_phi_accrual.toml` — mirrors `crash_recovery.toml` / `crash_adaptive.toml` / `crash_gossip.toml` exactly so the four detectors can be compared on identical conditions (same seed, same network, same fault schedule).

### Files modified
- `src/detector/mod.rs` — exposed the new module.
- `src/config.rs` — added `DetectorStrategy::PhiAccrual` plus three optional config fields (`phi_threshold`, `phi_window_size`, `phi_min_stddev`).
- `src/scenario.rs` — wired `PhiAccrual` into `build_engine`.
- `src/network.rs`, `tests/smoke.rs` — fixed two pre-existing `manual_range_contains` clippy errors that surfaced under the current Rust toolchain (unrelated to the new detector, but blocking the `clippy -D warnings` CI gate).

## Why φ-accrual specifically

The mentor's last email asked for benchmarks against the state of the art, in the repo, for reproducibility. φ-accrual is the canonical SOTA accrual detector — it is the reference baseline used in Cassandra and Akka and is the first detector reviewers in the distributed-systems community will look for. Implementing it before the meeting means we walk in with a concrete, working comparison rather than a plan.

This is **not** redundant with the existing `AdaptiveDetector`. Our `AdaptiveDetector` keeps an EWMA of inter-arrival times and applies a multiplicative safety factor — there is no probabilistic model and no continuous suspicion level. φ-accrual fits a Normal(μ, σ) to the recent inter-arrival window, computes P(next heartbeat arrives later than now), and reports a continuous suspicion level φ = −log₁₀(P_later). The `AdaptiveDetector` doc comment claims it is "inspired by" φ-accrual, which is fair, but it is not the algorithm reviewers will recognize when we say "phi accrual."

## Algorithm details

For each monitored node, we keep a sliding window of the last N inter-arrival times (default N=100). When `suspected_nodes()` is queried at tick *t*:

1. Δ = t − last_heartbeat_tick.
2. (μ, σ) ← mean and standard deviation of the window.
3. σ ← max(σ, min_stddev). The floor is essential — without it, a clean network produces a near-zero σ and any non-trivial Δ rounds to φ = ∞.
4. P_later = 1 − Φ((Δ − μ) / σ) where Φ is the standard normal CDF.
5. φ = −log₁₀(P_later). Suspect the node if φ > threshold.

We use the Abramowitz & Stegun 7.1.26 erf approximation (max error ~1.5×10⁻⁷) inline rather than pulling in `statrs` or `libm`, keeping the dependency footprint minimal as called out in the design doc.

Threshold defaults to 8.0, which is the aggressive setting from the original paper and Cassandra's default. The paper recommends 8 for snappy detection, 12 for moderate, 16 for conservative.

## Result — apples-to-apples on the crash scenario

Running all four detectors against the identical `crash_recovery.toml` setup (10 nodes, clean network, jitter=2, node 3 crashes at tick 2000 and recovers at 4000):

| Strategy        | Detections | False-positive rate | Mean detection latency (ticks) |
|-----------------|-----------:|--------------------:|-------------------------------:|
| Fixed-timeout   | 19         | 0.526               | 171.00                         |
| Adaptive (EWMA) | 19         | 0.526               | 171.00                         |
| Gossip          | 10         | 0.100               | 259.89                         |
| **φ-accrual**   | **19**     | **0.526**           | **71.00**                      |

The headline number is **2.4× lower detection latency at matched precision** vs. the fixed-timeout and EWMA-adaptive baselines. Gossip remains best on FP rate (because its threshold-of-3 confirmations damps spurious local suspicions) but pays for it in latency.

This single result already gives us one publishable plot — a latency-vs-FP-rate scatter across strategies — and the mentor can see the SOTA comparison is wired and reproducible from a committed config + seed.

## Caveats / open issues for the meeting

- **All four detectors fire spuriously on recovery** because `active_suspicions` clears on `NodeRecover` but each detector independently re-evaluates before the recovered node's first heartbeat arrives. This inflates the FP rate uniformly and is not a φ-accrual issue. Worth discussing with the mentor whether this counts as "misclassification" in the experimental framing or whether we should mask the post-recovery window.
- **Single seed.** Per `NEXT.md` priority 2, we still need multi-seed runs before any of these numbers can carry confidence intervals.
- **Single scenario.** This shows the strategy works and is competitive on a clean-network crash; we have not yet swept jitter, drops, or partitions against φ-accrual.

## Status

- `cargo fmt -- --check` — clean
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — 21 unit + 9 integration tests pass (6 new for φ-accrual)
- `cargo run --release -- --config configs/scenarios/crash_phi_accrual.toml` — runs end to end and exports `detections.csv` + `summary.csv`

## Memory

This is the first SOTA baseline in the repo. Next likely additions: SWIM (Das/Gupta/Motivala 2002) for a true gossip-protocol baseline distinct from our `GossipDetector`, and possibly Chen–Toueg–Aguilera adaptive (2002). Confirm priority with mentor before building.
