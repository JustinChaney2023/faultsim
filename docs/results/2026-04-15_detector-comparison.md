# Detector Comparison — 2026-04-15

Head-to-head comparison of five failure detectors on identical scenarios. Two SOTA baselines were added today: φ-accrual (Hayashibara 2004) and Satzger's adaptive accrual (2007).

All runs are reproducible — config + RNG seed are committed. To regenerate every number in this document:

```bash
cargo build --release
for cfg in configs/scenarios/crash_*.toml configs/scenarios/high_jitter*.toml; do
  ./target/release/faultsim --config "$cfg" \
    --output "results/comparison_2026-04-15/$(basename "$cfg" .toml)"
done
```

Single seed (42) per scenario. **Multi-seed runs with confidence intervals are still required before any of these numbers can ship in a paper** — they're enough to inform the meeting, not the manuscript.

## Detectors under test

| Strategy | Year | Reference | Mechanism |
|---|---|---|---|
| Fixed-timeout | — | textbook | Suspect if Δ > timeout |
| EWMA-adaptive | — | inspired by φ-accrual | Suspect if Δ > EWMA(intervals) × safety |
| Gossip | — | this project | Local timeout + threshold-of-K confirmations from peers |
| **φ-accrual** | 2004 | Hayashibara et al., SRDS | Suspect if −log₁₀(P_later) > θ; Normal(μ,σ) fit on window |
| **Adaptive-accrual** | 2007 | Satzger et al., SAC | Same suspicion formula; **empirical CDF** instead of Normal fit |

Configurations are identical across detectors within each scenario block (same cluster size, network parameters, seed, and fault schedule). Only the `[detector]` block changes.

## Scenario A — single crash + recovery, clean network

`crash_*.toml`. 10 nodes, base_latency=10, jitter=2, drop=0%. Node 3 crashes at tick 2000, recovers at tick 4000. Run for 10000 ticks.

| Strategy        | Detections | FP rate | Mean detection latency | Messages/tick |
|-----------------|-----------:|--------:|-----------------------:|--------------:|
| Fixed-timeout   | 19         | 0.526   | 171 ticks              | 0.88          |
| EWMA-adaptive   | 19         | 0.526   | 171 ticks              | 0.88          |
| Gossip          | 10         | 0.100   | 260 ticks              | 0.98          |
| **φ-accrual**       | 19         | 0.526   | **71 ticks**           | 0.88          |
| **Adaptive-accrual** | 20     | 0.550   | **71 ticks**           | 0.88          |

**Headline:** both accrual detectors cut detection latency from 171 → 71 ticks (**2.4× faster**) at essentially the same precision as fixed-timeout and EWMA-adaptive. Gossip remains the FP-rate winner because it requires K=3 peer confirmations, but pays ~3.7× in latency vs. the accrual detectors.

The 0.526 FP rate across four of five detectors is dominated by post-recovery false positives: when node 3 comes back at tick 4000, every other detector still has a stale "no recent heartbeat" view until the recovered node's first heartbeat arrives. This is a property of the simulation harness (per-detector independent re-evaluation) more than of the detection strategies. **Worth discussing with the mentor whether to mask the post-recovery window in the experimental framing.**

φ-accrual and Satzger 2007 are essentially tied here — expected, since the inter-arrival distribution under jitter=2 is approximately Normal, which is exactly what φ-accrual assumes.

## Scenario B — high jitter, no faults (pure false-positive stress test)

`high_jitter*.toml`. 10 nodes, base_latency=50, jitter=38 (76% of latency), drop=0%, **no crashes**. Every detection event in this scenario is by definition a false positive.

| Strategy        | Detections | FP rate | Verdict |
|-----------------|-----------:|--------:|---------|
| Fixed-timeout (timeout=200)  | 10 | 1.000 | ❌ saturates |
| EWMA-adaptive (α=0.5, ×2)    | 10 | 1.000 | ❌ saturates |
| Gossip                       |  0 | 0.000 | ✅ clean    |
| φ-accrual (θ=8)              |  0 | 0.000 | ✅ clean    |
| Adaptive-accrual (θ=2, n=100)| 11 | 1.000 | ❌ saturates |
| Adaptive-accrual (θ=3, n=100)| 11 | 1.000 | ❌ still saturates |
| Adaptive-accrual (θ=2, n=1000)| 11| 1.000 | ❌ still saturates (window doesn't fill) |

**Headline:** φ-accrual and gossip are robust to high jitter; the other three are not.

**The negative result on Satzger 2007 is the most interesting finding of the day** — and it's not a bug. The empirical-CDF approach has a hard upper bound of φ ≤ log₁₀(n) inside the observed range. With n=100 the ceiling is exactly 2.0. Past that, φ saturates to infinity as soon as Δ exceeds the largest observed inter-arrival. Under high jitter the largest observed inter-arrival is around 176 ticks (= heartbeat_interval + 2·jitter), and any Δ > 176 fires. φ-accrual, in contrast, extrapolates beyond the observed range via its Normal model — Δ has to exceed roughly μ + 5.6·σ ≈ 220 ticks to clear threshold 8, comfortably above the jitter envelope.

**Implication for the paper:** Satzger's empirical-CDF detector is supposed to win when the inter-arrival distribution is **non-Normal** (heavy-tailed, bimodal). On this Normal-jitter scenario, φ-accrual's parametric model is the right inductive bias and Satzger pays for its generality. Phase 2 of [the experiment plan](../experiment-plan.md) calls for sweeping jitter distributions; that sweep is exactly the right place to stress-test the comparison.

## Cross-cutting observations

1. **Both accrual detectors are 2.4× faster than fixed-timeout / EWMA-adaptive** at matched precision on a clean crash scenario.
2. **Gossip and φ-accrual are the only detectors that survive 75% jitter** without saturating false positives. Different mechanisms (peer confirmation vs. probabilistic suspicion) converge on robustness.
3. **Newer ≠ better in all conditions.** Satzger 2007 is meaningfully more general than φ-accrual but pays for it on Normal-jitter scenarios. This is the kind of empirical contradiction a measurements journal will value.
4. **Single-detector tuning matters as much as algorithm choice.** The Satzger ceiling-at-log₁₀(n) effect is a tuning gotcha that any benchmark must control for.

## Open questions for mentor (2026-04-16 meeting)

- Does the SOTA benchmark suite need SWIM as well, or are the two accrual detectors enough? SWIM (Das, Gupta, Motivala 2002) is older than φ-accrual but uses an indirect-probe mechanism distinct from anything we have today; its 2018 successor Lifeguard is a stronger newer baseline if the journal target is a distributed-systems venue.
- Should the experimental framing exclude the post-recovery window (where every per-detector view is briefly stale)? Including it inflates the headline FP numbers uniformly; excluding it isolates the detector logic.
- How many seeds per scenario for the manuscript runs? The numbers above are single-seed.
- Phase 2 of the experiment plan calls for non-Normal jitter (uniform / normal / Pareto). The simulator is currently uniform-only; adding Normal and Pareto in [src/network.rs](../../src/network.rs) is the prerequisite for the Satzger-vs-φ-accrual showdown. Worth doing before next session?

## Raw outputs

Per-scenario directories under [`results/comparison_2026-04-15/`](../../results/comparison_2026-04-15/) contain `detections.csv` and `summary.csv` for each run. The directory is git-ignored (regenerable from configs + seeds), but the configs themselves are committed under [`configs/scenarios/`](../../configs/scenarios/).
