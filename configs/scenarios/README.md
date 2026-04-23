# Scenario Configuration Reference

This directory contains the 18 TOML scenario files used to evaluate FaultSim's
failure-detection strategies. Each file is a complete, self-contained experiment:
run it with `faultsim run --config <file>` and results are written to
`results/<scenario-name>/` automatically.

---

## Scenario Index

| File | Strategy | Network | Faults | Purpose |
|---|---|---|---|---|
| `baseline.toml` | FixedTimeout | Clean | None | Zero-fault reference; FP should be 0 |
| `adaptive.toml` | Adaptive | Clean | None | EWMA baseline with no faults |
| `gossip.toml` | Gossip | Clean | None | Gossip baseline with no faults |
| `crash_recovery.toml` | FixedTimeout | Clean | Crash + Recover | Canonical crash/recovery reference |
| `crash_adaptive.toml` | Adaptive | Clean | Crash + Recover | EWMA on crash/recovery |
| `crash_gossip.toml` | Gossip | Clean | Crash + Recover | Gossip on crash/recovery |
| `crash_phi_accrual.toml` | φ-Accrual | Clean | Crash + Recover | Hayashibara 2004 on crash/recovery |
| `crash_adaptive_accrual.toml` | Adaptive Accrual | Clean | Crash + Recover | Satzger 2007 on crash/recovery |
| `drops_5pct.toml` | FixedTimeout | 5% drop | None | Mild packet loss stress test |
| `drops_15pct.toml` | FixedTimeout | 15% drop | None | Severe packet loss stress test |
| `high_jitter.toml` | FixedTimeout | High jitter | None | Jitter sensitivity baseline |
| `high_jitter_adaptive.toml` | Adaptive | High jitter | None | EWMA under high jitter |
| `high_jitter_gossip.toml` | Gossip | High jitter | None | Gossip under high jitter |
| `high_jitter_phi_accrual.toml` | φ-Accrual | High jitter | None | φ-accrual under high jitter |
| `high_jitter_adaptive_accrual.toml` | Adaptive Accrual | High jitter | None | Satzger 2007 under high jitter |
| `high_jitter_adaptive_accrual_t3.toml` | Adaptive Accrual | High jitter | None | Same, threshold=3.0 (tuning probe) |
| `high_jitter_adaptive_accrual_w1000.toml` | Adaptive Accrual | High jitter | None | Same, window=1000 (wider history) |
| `partition.toml` | FixedTimeout | Clean | Partition | Network split, FP stress test |

---

## Parameter Reference

### `[simulation]`

| Field | Type | Default | Description |
|---|---|---|---|
| `max_ticks` | `u64` | — | Simulation stops after this many ticks. One tick = one logical time unit. With `heartbeat_interval = 100`, `max_ticks = 10000` gives ~100 heartbeat rounds. |
| `seed` | `u64` | — | RNG seed for deterministic replay. Same seed + same config always produces the same run. Override at the CLI with `--seed`. |

---

### `[cluster]`

| Field | Type | Units | Description |
|---|---|---|---|
| `node_count` | `u32` | nodes | Number of nodes in the simulated cluster. Each node monitors all others. Message count grows as O(n²) per heartbeat round. |
| `heartbeat_interval` | `u64` | ticks | Ticks between heartbeat sends. Also the detector polling interval. Typical: 100 ticks. Detection latency is bounded below by this value. |

---

### `[network]`

| Field | Type | Units | Range | Description |
|---|---|---|---|---|
| `base_latency` | `u64` | ticks | ≥ 1 | Base one-way message delivery delay. Added to every delivered message. |
| `jitter` | `u64` | ticks | 0 – `base_latency` | Uniform random additional delay in [0, jitter]. Jitter ≥ 75% of base_latency is "high jitter" and stresses all threshold-based detectors. |
| `drop_probability` | `f64` | — | 0.0 – 1.0 | Fraction of messages silently dropped. 0.05 (5%) is mild stress; 0.15 (15%) causes frequent spurious timeouts on FixedTimeout and Adaptive detectors. |

---

### `[detector]`

#### Common

| Field | Type | Description |
|---|---|---|
| `strategy` | enum | One of: `fixed_timeout`, `adaptive`, `gossip`, `phi_accrual`, `adaptive_accrual`. |

#### `strategy = "fixed_timeout"`

Binary suspicion: a node is suspected the first time `heartbeat_interval + timeout` ticks pass without a heartbeat.

| Field | Type | Units | Recommended | Description |
|---|---|---|---|---|
| `timeout` | `u64` | ticks | 1–3× `heartbeat_interval` | Ticks of silence beyond one heartbeat interval before declaring suspicion. Lower → faster detection, higher FP rate under jitter or drops. |

#### `strategy = "adaptive"`

EWMA-based adaptive timeout. Tracks the moving average of inter-arrival times and sets a timeout of `alpha_mean * safety_multiplier`.

| Field | Type | Range | Recommended | Description |
|---|---|---|---|---|
| `alpha` | `f64` | 0.0 – 1.0 | 0.3 – 0.7 | EWMA smoothing factor. Higher α weights recent samples more; lower α is more stable but slower to adapt. |
| `safety_multiplier` | `f64` | > 1.0 | 1.5 – 3.0 | Multiplier applied to the EWMA mean to form the timeout. Lower → faster detection and higher FP rate. |

#### `strategy = "gossip"`

Local timeout combined with multi-source corroboration. A node is suspected only after `suspicion_threshold` independent observers report suspicion via gossip.

| Field | Type | Units | Recommended | Description |
|---|---|---|---|---|
| `timeout` | `u64` | ticks | Same as FixedTimeout | Local silence threshold before a node raises a local suspicion. |
| `suspicion_threshold` | `u32` | — | 2 – 5 | Number of independent gossip reports required to confirm suspicion. Higher → fewer FPs, higher latency. |
| `gossip_interval` | `u64` | ticks | 20 – 100 | Ticks between gossip rounds. Lower → suspicion propagates faster but message overhead increases. |
| `gossip_fanout` | `u32` | peers | 2 – 5 | Number of random peers contacted per gossip round. Higher → faster propagation, more messages. |

#### `strategy = "phi_accrual"`

Continuous suspicion level φ modelled as −log₁₀(P_later) where P_later is computed under a Normal(μ, σ) fit to recent inter-arrival times. A node is suspected when φ exceeds `phi_threshold`.

Reference: Hayashibara, Défago, Yared, Katayama — *The φ Accrual Failure Detector*, SRDS 2004. Default in Apache Cassandra and Akka.

| Field | Type | Range | Recommended | Description |
|---|---|---|---|---|
| `phi_threshold` | `f64` | > 0 | 8 (aggressive) / 12 / 16 (conservative) | Suspicion threshold. φ = 8 ≈ P(false alarm) = 10⁻⁸ under the Normal model. Higher → fewer FPs but slower detection. The Hayashibara paper recommends 8 for most deployments. |
| `phi_window_size` | `usize` | samples | 50 – 1000 | Sliding window of inter-arrival samples used to fit the Normal distribution. Larger windows are more stable but slower to adapt to regime changes. |
| `phi_min_stddev` | `f64` | ticks | 1.0 – heartbeat_interval | Floor on the estimated stddev. Prevents φ from blowing up on perfectly regular networks where the fitted stddev would be ~0. |

#### `strategy = "adaptive_accrual"`

Like φ-accrual but replaces the Normal distribution assumption with an empirical CDF of the inter-arrival window. φ = −log₁₀(fraction of historical intervals ≥ Δ). More robust to heavy-tailed or bimodal jitter distributions, but φ is bounded by −log₁₀(1/(n+1)) so `phi_threshold` must be set lower.

Reference: Satzger, Pietzowski, Trumler, Ungerer — *A new adaptive accrual failure detector for dependable distributed systems*, SAC 2007.

| Field | Type | Range | Recommended | Description |
|---|---|---|---|---|
| `phi_threshold` | `f64` | > 0 | 2.0 – 4.0 | Suspicion threshold. With window=100, the maximum φ is log₁₀(101) ≈ 2.0, so threshold must be < this. With window=1000, maximum ≈ 3.0. |
| `phi_window_size` | `usize` | samples | 100 – 1000 | Sliding window size. Also controls the ceiling on φ: max φ = log₁₀(window_size + 1). Larger windows raise the ceiling and allow more sensitivity. |

---

### `[[faults]]`

Zero or more fault injection events, executed at a specified tick. Listed in the order they should fire; typically sorted by `tick`.

| Field | Type | Description |
|---|---|---|
| `tick` | `u64` | Tick at which this fault fires. |
| `kind` | enum | One of: `crash`, `recover`, `partition_start`, `partition_end`. |
| `node` | `u64` | Target node ID (required for `crash` and `recover`). Node IDs are 1-indexed up to `node_count`. |
| `groups` | `[[u64]]` | Required for `partition_start`. Each inner array is a group of node IDs that can communicate internally but not cross-group. |

Example — crash at tick 2000, recover at tick 4000:
```toml
[[faults]]
tick = 2000
kind = "crash"
node = 3

[[faults]]
tick = 4000
kind = "recover"
node = 3
```

Example — network partition at tick 2000, healed at tick 5000:
```toml
[[faults]]
tick = 2000
kind = "partition_start"
groups = [[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]]

[[faults]]
tick = 5000
kind = "partition_end"
```

---

### `[output]`

| Field | Type | Default | Description |
|---|---|---|---|
| `dir` | `string` | — | Directory for result files. Created automatically. Overridable at CLI with `--output`. |
| `format` | `string` | `"csv"` | Export format: `"csv"` or `"json"`. CSV is recommended for batch analysis with `run-all`. JSON is convenient for individual runs consumed by other tooling. |
| `phi_log` | `bool` | `false` | When `true`, records φ values from accrual detectors on every `DetectorTick` and writes `<name>_phi_log.csv` and `<name>_events.csv`. Only meaningful for `phi_accrual` and `adaptive_accrual` strategies; silently no-ops for others. Large simulations may produce hundreds of thousands of φ log rows. |

---

## Quick-start: adding a new scenario

1. Copy the closest existing scenario file.
2. Change `[detector] strategy` and parameters as needed.
3. Set `[output] dir = "results/<your-scenario-name>"`.
4. Run: `faultsim run --config configs/scenarios/<your-file>.toml`
5. Compare against baseline using `run-all` and `scripts/plot_results.py`.

To add a new detector strategy, implement the `FailureDetector` trait in `src/detector/` and wire it into `src/scenario.rs`.
