# 2026-04-15 — Add Satzger 2007 Adaptive Accrual Detector

## What changed

Added Satzger et al.'s adaptive accrual failure detector (SAC 2007) as a fifth detection strategy. Builds on today's earlier φ-accrual addition and gives us a "newer SOTA" to compare against the canonical 2004 baseline.

### Files added
- `src/detector/adaptive_accrual.rs` — empirical-CDF based accrual detector. 6 unit tests including a bimodal-distribution test that φ-accrual would handle poorly.
- `configs/scenarios/crash_adaptive_accrual.toml` — mirrors `crash_phi_accrual.toml` for direct head-to-head.
- `configs/scenarios/high_jitter_gossip.toml`, `high_jitter_phi_accrual.toml`, `high_jitter_adaptive_accrual.toml` — fills out the high-jitter sweep so all five detectors can be compared under the FP-stress scenario. Plus two threshold/window-tuning variants of the Satzger config.
- `docs/results/2026-04-15_detector-comparison.md` — first full results document with two scenarios × five detectors and the reproducibility recipe.

### Files modified
- `src/detector/mod.rs`, `src/config.rs`, `src/scenario.rs` — wired the new strategy in. Reuses the existing `phi_threshold` / `phi_window_size` config fields (semantics are the same; recommended values differ).

## Why Satzger 2007 over alternatives

The mentor asked specifically for "a newer one" relative to φ-accrual (2004). Candidates considered:

- **SWIM (2002)** — older than φ-accrual, ruled out on the "newer" criterion. Worth implementing later as a different *paradigm* (indirect probing) but doesn't fit today's brief.
- **Bertier et al. (2003)** — older.
- **Satzger et al. (2007)** — direct improvement on φ-accrual, replaces the Normal-distribution assumption with an empirical CDF. Same API, same suspicion formula, easy apples-to-apples comparison. **Chosen.**
- **NFD-A (Xiong et al. 2012)** — extends Satzger with sliding-window enhancements. Worth doing later; more complex to implement correctly and the 2007 version is the cleaner story for a first pass.
- **Lifeguard (Dadgar et al. 2018)** — extends SWIM, requires SWIM as a base. Future work.
- **Rapid (Suresh et al. 2018)** — consensus-based membership, different paradigm. Probably overkill for a first SOTA pass.

Satzger 2007 also pairs naturally with Phase 2 of the experiment plan (jitter distributions): its empirical CDF should outperform φ-accrual specifically when the inter-arrival distribution is non-Normal.

## Algorithm

For each monitored node, maintain a sliding window of inter-arrival times. When `suspected_nodes()` is queried at tick *t*:

1. Δ = t − last_heartbeat_tick.
2. P_later(Δ) = |{s ∈ window : s ≥ Δ}| / |window|. **No parametric fit.**
3. If P_later = 0 (Δ exceeds every observed sample), saturate to φ = ∞.
4. Otherwise φ = −log₁₀(P_later). Suspect if φ > threshold.

Initial implementation used add-one smoothing (Laplace-style) on the count to avoid the infinity case. Removed it after the long-silence test failed at φ=1.30 instead of >2.0 — turns out Satzger's actual formulation is the pure empirical fraction, with explicit saturation at the boundary as a documented limitation. Kept that and added a comment.

## Result — apples-to-apples on two scenarios

Full table in [docs/results/2026-04-15_detector-comparison.md](../results/2026-04-15_detector-comparison.md). Headlines:

**Crash + recovery, clean network:** Satzger and φ-accrual are tied — both achieve 71-tick mean detection latency vs. 171 for fixed-timeout / EWMA-adaptive (2.4× faster) at essentially the same FP rate. Expected, because the inter-arrival distribution under jitter=2 is approximately Normal — exactly φ-accrual's assumption.

**High jitter (76% of base latency), no faults:**

| Strategy | FP rate |
|---|---:|
| Fixed-timeout | 1.000 ❌ |
| EWMA-adaptive | 1.000 ❌ |
| Gossip | 0.000 ✅ |
| φ-accrual | 0.000 ✅ |
| **Satzger 2007** | **1.000 ❌** |

This is the most interesting result of the day: **the "newer" detector regresses on this scenario**. Not a bug — a fundamental property of empirical-CDF accrual detectors. With window size n, φ has a hard ceiling of log₁₀(n) inside the observed range, then saturates to ∞ once Δ exceeds the maximum observed sample. With n=100 the ceiling is exactly 2.0; threshold values at or above that effectively reduce to "fire when Δ exceeds max observed inter-arrival." Under high jitter the max observed inter-arrival is around 176 ticks, and Δ values just above that are common and benign.

φ-accrual's Normal model extrapolates beyond the observed range, requiring Δ ≈ μ + 5.6σ ≈ 220 ticks before threshold 8 fires — comfortably above the jitter envelope.

Tried two mitigations — threshold=3 (still saturates because 3 > log₁₀(100)) and window_size=1000 (doesn't help because the simulation only generates ~100 heartbeats per node). The behavior is structural, not tunable away in this scenario.

**Implication for the manuscript:** Satzger should win where φ-accrual loses — under non-Normal jitter (heavy-tailed, bimodal). The simulator's `network.rs` currently only supports uniform jitter; adding Normal and Pareto is the prerequisite for actually demonstrating Satzger's intended advantage.

## Caveats / open issues

Same as the φ-accrual log:
- Single-seed runs only.
- Post-recovery FPs dominate the crash-scenario FP rate uniformly across detectors. Need to decide whether to mask that window in the experimental framing.

Plus newly surfaced:
- The Satzger detector's empirical-CDF ceiling (φ ≤ log₁₀(n) inside observed range) is a structural property worth calling out explicitly in any paper that includes it. **Do not present a Satzger result without also reporting (n, threshold) and noting the ceiling.**

## Status

- `cargo fmt -- --check` — clean
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — 27 unit + 9 integration tests pass (6 new for adaptive-accrual)
- All 12 scenario configs run end to end and export CSVs.

## Memory

Today's two log entries (φ-accrual + this one) together constitute the first end-to-end SOTA-comparison story in the repo. With the results document at `docs/results/2026-04-15_detector-comparison.md`, this is enough material for the 2026-04-16 mentor meeting. Next session should focus on (a) confirming the SOTA shortlist with the mentor and (b) closing the multi-seed / non-Normal-jitter gaps that block honest Satzger-vs-φ-accrual claims.
