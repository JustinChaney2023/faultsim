# Simulator Reference

This document describes everything the faultsim simulator does and how it works.

## What the Simulator Does

faultsim is a discrete-event simulator that models a cluster of nodes exchanging heartbeat messages over a simulated network. It measures how well different failure-detection strategies distinguish between actual node failures and transient network problems (jitter, packet loss, partitions).

The simulator answers: **under what conditions do failure detectors misclassify healthy nodes as failed?**

It produces four key metrics for each run:
- **Detection latency** — ticks between an actual crash and the detector flagging it
- **False positive rate** — fraction of detection events where the suspected node was actually alive
- **Messages per tick** — average heartbeat messages delivered per simulation tick
- **Crash/recovery counts** — how many fault injection events occurred

## How Simulation Time Works

All time is measured in **ticks** (unsigned 64-bit integers). There is no wall-clock coupling — the simulation clock only advances when the engine processes an event. This means simulations are instant regardless of how many ticks they cover.

Given the same configuration and RNG seed, a simulation produces **identical results every time** (deterministic replay).

## Components

### Simulation Engine (`src/engine.rs`)

The engine is the main loop. It owns all simulation state:
- A **clock** (current tick)
- An **event queue** (min-heap priority queue ordered by tick)
- A **node table** (HashMap of NodeId → Node)
- A **network** model
- A **detector table** (HashMap of NodeId → Box\<dyn FailureDetector\>)
- A **metrics collector**
- A seeded **RNG** (StdRng)

The loop:
1. Pop the earliest event from the queue
2. If its tick exceeds `max_ticks`, stop
3. Advance the clock to that tick
4. Dispatch the event to the appropriate handler

### Event Types (`src/event.rs`)

| Event | What happens |
|---|---|
| `HeartbeatSend { from }` | Node sends a heartbeat to each of its peers via the network. The network computes a delivery tick (or drops the message). A `HeartbeatArrival` event is scheduled for each successfully sent message. The next `HeartbeatSend` is re-scheduled at `current_tick + heartbeat_interval`. |
| `HeartbeatArrival { from, to }` | A heartbeat message arrives at the receiver. If the receiver is alive, its failure detector's `on_heartbeat(from, tick)` is called. |
| `NodeCrash { node }` | The node's state is set to `Crashed`. It stops sending heartbeats (the `HeartbeatSend` handler checks aliveness). |
| `NodeRecover { node }` | The node's state is set to `Alive`. Heartbeat sends and detector ticks resume immediately. Active suspicions targeting this node are cleared. |
| `DetectorTick { node }` | The node's failure detector runs: `on_tick(tick)` is called, then `suspected_nodes()` is queried. Each newly suspected node is classified as a true positive (the node is actually crashed) or false positive (the node is alive), and a detection event is recorded. The next `DetectorTick` is re-scheduled. |
| `GossipRound { from }` | A node initiates a gossip round: picks `gossip_fanout` random peers, sends them its local suspicion list via the network. Messages are subject to latency, drops, and partitions. The next `GossipRound` is re-scheduled at `current_tick + gossip_interval`. |
| `GossipArrival { from, to, suspected }` | A gossip message arrives carrying a list of suspected nodes. Each suspected node is recorded as a suspicion from the sender on the receiver's gossip detector (duplicate sources are ignored). |
| `PartitionStart { groups }` | A network partition is applied. Nodes in different groups cannot communicate — messages between groups are silently dropped. |
| `PartitionEnd` | All partition rules are removed. Cross-group communication resumes. |

### Nodes (`src/node.rs`)

Each node has:
- A unique `NodeId` (u64, numbered 1..=N)
- A `state` (Alive or Crashed)
- A `heartbeat_interval` — ticks between heartbeat sends
- A `detector_interval` — ticks between detector checks
- A `peers` list — all other node IDs in the cluster

### Network (`src/network.rs`)

The network model determines whether a message is delivered and when. For each message:

1. **Partition check**: if the (sender, receiver) pair is in the blocked set, the message is dropped
2. **Drop check**: with probability `drop_probability`, the message is dropped
3. **Latency**: delivery tick = `send_tick + base_latency + uniform_random(0..=jitter)`

Partitions are **symmetric by default** — when `apply_partition` is called with groups, all cross-group pairs are blocked in both directions.

### Failure Detectors (`src/detector/`)

All detectors implement the `FailureDetector` trait:

```rust
pub trait FailureDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick);
    fn on_tick(&mut self, tick: Tick);
    fn suspected_nodes(&self) -> Vec<NodeId>;
}
```

#### Fixed-Timeout Detector (`fixed_timeout.rs`)

The simplest strategy. Suspects a node if no heartbeat has been received within a fixed number of ticks (`timeout`).

**Parameters:**
- `timeout` — ticks without a heartbeat before suspicion (default: 200)

**Behavior:** On each detector tick, for each monitored node: if `current_tick - last_heartbeat > timeout`, the node is suspected.

#### Adaptive Detector (`adaptive.rs`)

Dynamically adjusts its timeout based on observed heartbeat inter-arrival times using an exponentially weighted moving average (EWMA). Inspired by the Phi Accrual detector and TCP RTT estimation.

**Parameters:**
- `alpha` — EWMA smoothing factor, 0 < alpha ≤ 1 (default: 0.5). Higher values weight recent samples more heavily.
- `safety_multiplier` — multiplied against the EWMA to get the actual timeout (default: 2.0)

**Behavior:** On each heartbeat arrival, the inter-arrival delta is computed and the EWMA is updated: `ewma = alpha * delta + (1 - alpha) * ewma`. The dynamic timeout is `ewma * safety_multiplier`. A node is suspected if `current_tick - last_heartbeat > dynamic_timeout`. Before any heartbeats are observed, a default EWMA of 100 ticks is used.

#### Gossip Detector (`gossip.rs`)

Combines local heartbeat monitoring with aggregated suspicion counts. A node is only declared failed when multiple independent sources report missed heartbeats. Inspired by SWIM and Lifeguard protocols.

**Parameters:**
- `suspicion_threshold` — number of independent suspicions required to declare failure (default: 3)
- `local_timeout` — ticks without a heartbeat before local suspicion fires (reuses the `timeout` config field)
- `gossip_interval` — ticks between gossip rounds (default: 50)
- `gossip_fanout` — number of random peers to gossip with each round (default: 3)

**Behavior:** On each detector tick, if a monitored node has missed heartbeats beyond `local_timeout`, the local suspicion count is incremented (once per missed-heartbeat episode). Periodically (every `gossip_interval` ticks), a node initiates a gossip round: it picks `gossip_fanout` random peers and sends them its local suspicion list via the network (subject to latency, drops, and partitions). When a gossip message arrives, each suspected node in the list is recorded as a suspicion from the sender — but duplicate sources are not double-counted. When `suspicion_count >= suspicion_threshold`, the node is declared suspected. A direct heartbeat clears all suspicion and source tracking for that node.

### Detection Deduplication

The engine tracks an `active_suspicions` set of `(detector_node, suspected_node)` pairs. A detection event is only recorded the **first time** a pair appears. This prevents the same suspicion from generating repeated events on every detector tick. The pair is cleared when:
- The suspected node recovers
- The detector no longer suspects that node

### Metrics Collector (`src/metrics.rs`)

Records all simulation events for post-run analysis:

| Field | Type | Description |
|---|---|---|
| `message_count` | u64 | Total heartbeat messages successfully delivered |
| `detections` | Vec\<DetectionEvent\> | Each detection: tick, node, true_positive flag, latency |
| `crashes` | Vec\<(Tick, NodeId)\> | Every crash injection event |
| `recoveries` | Vec\<(Tick, NodeId)\> | Every recovery event |

**Computed statistics:**
- `false_positive_rate()` — false positives / total detections
- `mean_detection_latency()` — average ticks from crash to detection (true positives only)
- `messages_per_tick(total_ticks)` — message_count / total_ticks

**CSV export:**
- `export_detections_csv(path)` — writes per-event CSV: tick, node, true_positive, latency
- `export_summary_csv(path, total_ticks, scenario_name)` — appends a summary row to a CSV file

## Configuration Format

Scenarios are defined in TOML files. See `configs/scenarios/` for examples.

### Required sections

```toml
[simulation]
max_ticks = 10000       # How long the simulation runs
seed = 42               # RNG seed for deterministic replay

[cluster]
node_count = 10         # Number of nodes in the cluster
heartbeat_interval = 100 # Ticks between heartbeat sends

[network]
base_latency = 10       # Base one-way message latency in ticks
jitter = 2              # Maximum random jitter added to latency
drop_probability = 0.0  # Probability [0.0, 1.0] that a message is dropped

[detector]
strategy = "fixed_timeout"  # "fixed_timeout", "adaptive", or "gossip"
```

### Detector-specific parameters

```toml
# Fixed timeout
timeout = 200

# Adaptive
alpha = 0.5
safety_multiplier = 2.0

# Gossip
timeout = 200               # Local timeout for missed heartbeats
suspicion_threshold = 3      # Independent suspicions needed
gossip_interval = 50         # Ticks between gossip rounds
gossip_fanout = 3            # Peers to gossip with each round
```

### Fault injection (optional)

```toml
[[faults]]
tick = 2000
kind = "crash"          # "crash", "recover", "partition_start", "partition_end"
node = 3                # Required for crash/recover

[[faults]]
tick = 5000
kind = "partition_start"
groups = [[1, 2, 3], [4, 5, 6, 7]]  # Required for partition_start

[[faults]]
tick = 8000
kind = "partition_end"  # Lifts all active partitions
```

### Output (optional)

```toml
[output]
dir = "results/my_experiment"
```

## Running Experiments

### Single scenario

```bash
cargo run -- --config configs/scenarios/baseline.toml
```

### With seed override

```bash
cargo run -- --config configs/scenarios/baseline.toml --seed 99
```

### With CSV export

```bash
cargo run -- --config configs/scenarios/baseline.toml --output results/run1
```

This writes `detections.csv` and `summary.csv` to the output directory.

### Batch run

```bash
./scripts/run_experiment.sh configs/scenarios/*.toml
```

### Generate plots

After running scenarios with `--output`, generate comparison charts:

```bash
# Run all scenarios and export CSVs
for f in configs/scenarios/*.toml; do
    name=$(basename "$f" .toml)
    cargo run -- --config "$f" --output "results/$name"
done

# Generate plots from all summary CSVs
python scripts/plot_results.py results/*/summary.csv --output results/plots
```

This produces 5 charts in `results/plots/`:
- `false_positive_rates.png` — FP rate across all scenarios
- `detection_latency.png` — detection latency for crash scenarios
- `detection_counts.png` — true vs false positive breakdown
- `messages_per_tick.png` — messaging overhead comparison
- `strategy_comparison.png` — side-by-side comparison of the three detector strategies on the crash scenario

Requires matplotlib: `pip install matplotlib`

### Available scenarios

| File | Description |
|---|---|
| `baseline.toml` | Clean network, no faults, fixed-timeout detector |
| `adaptive.toml` | Clean network, EWMA-based adaptive detector |
| `gossip.toml` | Clean network, gossip-assisted suspicion detector |
| `crash_recovery.toml` | Node 3 crashes at tick 2000, recovers at tick 4000 (fixed-timeout) |
| `crash_adaptive.toml` | Same crash scenario with adaptive detector |
| `crash_gossip.toml` | Same crash scenario with gossip detector |
| `high_jitter.toml` | 75% jitter relative to base latency (fixed-timeout) |
| `high_jitter_adaptive.toml` | Same high-jitter scenario with adaptive detector |
| `drops_5pct.toml` | 5% packet loss (fixed-timeout) |
| `drops_15pct.toml` | 15% packet loss (fixed-timeout) |
| `partition.toml` | Cluster splits into two groups at tick 2000, heals at tick 5000 |

## Test Examples

The following experiments were run with `seed = 42` and their results verified against expected behavior. Each test targets a specific property of the simulator or detector strategy. You can reproduce every result below by running the listed command.

---

### Test 1: Stable Network Baseline

**What we're testing:** In a clean network with no faults, no messages should be dropped and no node should ever be falsely suspected.

**Config:** `configs/scenarios/baseline.toml` — 10 nodes, heartbeat every 100 ticks, base latency 10, jitter 2, no drops, fixed-timeout detector with timeout=200.

```bash
cargo run -- --config configs/scenarios/baseline.toml
```

**Result:**
```
Messages delivered:     9000
Detection events:       0
False positive rate:    0.0000
```

**Analysis:** Accurate. Each of the 10 nodes sends heartbeats to 9 peers every 100 ticks over 10,000 ticks, producing 10 × 9 × (10000/100) = 9,000 messages. With latency 10–12 ticks and a timeout of 200 ticks, heartbeats always arrive well within the window. Zero false positives confirms the detector is correctly calibrated for this network.

---

### Test 2: Crash Detection Latency (Fixed-Timeout)

**What we're testing:** When a node crashes, how quickly does the fixed-timeout detector flag it? And what happens after recovery?

**Config:** `configs/scenarios/crash_recovery.toml` — same baseline, but node 3 crashes at tick 2000 and recovers at tick 4000.

```bash
cargo run -- --config configs/scenarios/crash_recovery.toml
```

**Result:**
```
Messages delivered:     8820
Crashes:                1
Recoveries:             1
Detection events:       19
False positive rate:    0.5263
Mean detection latency: 171.00 ticks
```

**Analysis:** Accurate. The detection latency of 171 ticks makes sense — after node 3's last heartbeat, each detector must wait for the 200-tick timeout to expire. The actual latency depends on where in the heartbeat cycle the crash occurs and when the next detector tick fires. 9 of 19 detections are true positives (the other 9 nodes each correctly suspect node 3). The remaining 10 are false positives that occur right after recovery at tick 4000 — there's a brief window where node 3 is alive again but hasn't yet sent heartbeats to clear suspicion. The 52.6% false positive rate reflects this transient misclassification during recovery.

Message count dropped to 8,820 because node 3 was silent for 2,000 ticks (2,000/100 × 9 = 180 fewer messages).

---

### Test 3: Crash Detection (Adaptive Detector)

**What we're testing:** Does the adaptive detector perform differently from fixed-timeout on the same crash scenario?

**Config:** `configs/scenarios/crash_adaptive.toml` — same crash/recovery schedule, but using the adaptive EWMA detector (alpha=0.5, safety_multiplier=2.0).

```bash
cargo run -- --config configs/scenarios/crash_adaptive.toml
```

**Result:**
```
Detection events:       19
False positive rate:    0.5263
Mean detection latency: 171.00 ticks
```

**Analysis:** Accurate. In a low-jitter network, the adaptive detector's EWMA converges to approximately the actual heartbeat interval (~100 ticks), so the dynamic timeout settles at ~200 ticks (100 × 2.0 safety multiplier) — effectively identical to the fixed timeout=200 config. The results match exactly, which confirms the EWMA is converging correctly. The adaptive detector's advantage would show in variable-jitter environments where a fixed timeout is either too tight (causing false positives) or too loose (slow detection).

---

### Test 4: Crash Detection (Gossip Detector)

**What we're testing:** Does the gossip detector's multi-source suspicion requirement improve accuracy over fixed-timeout?

**Config:** `configs/scenarios/crash_gossip.toml` — same crash/recovery schedule, gossip detector with suspicion_threshold=3, gossip_interval=50, gossip_fanout=3.

```bash
cargo run -- --config configs/scenarios/crash_gossip.toml
```

**Result:**
```
Messages delivered:     9810
Detection events:       10
False positive rate:    0.1000
Mean detection latency: 259.89 ticks
```

**Analysis:** Accurate, and this demonstrates the gossip detector's core tradeoff. Requiring 3 independent suspicion sources before declaring failure dramatically reduces false positives (10% vs 52.6% for fixed-timeout) but increases detection latency (260 vs 171 ticks). The higher latency occurs because suspicion must propagate through gossip rounds — each node detects the missing heartbeat locally, then gossips its suspicion list to 3 random peers every 50 ticks. Only after 3 independent sources confirm does the node get flagged. The message count is slightly higher (9,810 vs 8,820) due to gossip protocol overhead. Only 1 of 10 detections is a false positive, compared to 10 of 19 for fixed-timeout — the gossip detector's conservatism filters out the transient recovery-window misclassifications that plague the other strategies.

---

### Test 5: High Jitter with Fixed-Timeout

**What we're testing:** Does high network jitter cause the fixed-timeout detector to produce false positives on healthy nodes?

**Config:** `configs/scenarios/high_jitter.toml` — base latency 50, jitter 38 (75% of base), no crashes, fixed-timeout=200.

```bash
cargo run -- --config configs/scenarios/high_jitter.toml
```

**Result:**
```
Detection events:       10
False positive rate:    1.0000
```

**Analysis:** Accurate. With base latency 50 and jitter up to 38, heartbeats can arrive as late as tick 88 after sending. Heartbeats are sent every 100 ticks, so the inter-arrival time can be as high as 100 + 88 = 188 ticks. The timeout of 200 ticks is tight enough that occasional bursts of high-jitter heartbeats push past the threshold, producing 10 false positive events across the run. All detections are false positives (no nodes actually crashed). This demonstrates the fundamental weakness of fixed-timeout detectors in jittery networks.

---

### Test 6: High Jitter with Adaptive Detector

**What we're testing:** Does the adaptive detector handle the same high-jitter scenario better than fixed-timeout?

**Config:** `configs/scenarios/high_jitter_adaptive.toml` — same high-jitter network, adaptive detector.

```bash
cargo run -- --config configs/scenarios/high_jitter_adaptive.toml
```

**Result:**
```
Detection events:       10
False positive rate:    1.0000
```

**Analysis:** The adaptive detector also produces 10 false positives — matching fixed-timeout. This is because the adaptive detector's EWMA with alpha=0.5 and safety_multiplier=2.0 converges to a dynamic timeout roughly similar to the fixed 200-tick window. With heavier jitter, the EWMA tracks the mean inter-arrival time but individual samples can still exceed `ewma * 2.0`. A higher safety multiplier (e.g., 3.0 or 4.0) would reduce false positives at the cost of slower detection. This demonstrates that the adaptive detector's advantage depends on proper tuning — it's not automatically better than fixed-timeout.

---

### Test 7: Packet Loss Sensitivity

**What we're testing:** How does increasing packet drop rate affect false positives?

**Configs:** `configs/scenarios/drops_5pct.toml` (5% drops) and `configs/scenarios/drops_15pct.toml` (15% drops) — fixed-timeout=200.

```bash
cargo run -- --config configs/scenarios/drops_5pct.toml
cargo run -- --config configs/scenarios/drops_15pct.toml
```

**Results:**

| Drop rate | Messages delivered | Detection events | False positive rate |
|---|---|---|---|
| 0% (baseline) | 9,000 | 0 | 0.0000 |
| 5% | 8,558 | 21 | 1.0000 |
| 15% | 7,644 | 170 | 1.0000 |

**Analysis:** Accurate. At 5% drop rate, ~442 messages are lost (9000 × 0.049), and occasional consecutive drops for the same node push past the 200-tick timeout, causing 21 false positives. At 15%, message loss is severe enough (1,356 lost) that 170 false positives occur — the detector frequently believes healthy nodes have failed because multiple consecutive heartbeats are dropped. The message counts match expected values: 9000 × 0.95 ≈ 8,550 and 9000 × 0.85 ≈ 7,650. This confirms the network drop model is working correctly and demonstrates why fixed-timeout detectors are fragile under packet loss.

---

### Test 8: Network Partition

**What we're testing:** When the cluster splits into two groups, do nodes correctly (but falsely) suspect the other group?

**Config:** `configs/scenarios/partition.toml` — nodes [1-5] vs [6-10], partition at tick 2000, heals at tick 5000.

```bash
cargo run -- --config configs/scenarios/partition.toml
```

**Result:**
```
Messages delivered:     7500
Detection events:       50
False positive rate:    1.0000
```

**Analysis:** Accurate. During the 3,000-tick partition (ticks 2000–5000), each node can only reach 4 peers instead of 9. That means 5 × (3000/100) = 150 heartbeats per node are blocked cross-group. Message count drops from 9,000 to 7,500, matching: 9000 - (10 nodes × 5 blocked peers × 30 heartbeat cycles) = 9000 - 1500 = 7500. Each of the 10 nodes suspects the 5 nodes in the other group, producing 50 false positive detection events (10 × 5 = 50). All are false positives because no node actually crashed — they simply can't communicate across the partition. After the partition heals at tick 5000, heartbeats resume and suspicions clear.

---

### Test 9: Deterministic Replay

**What we're testing:** Does the same config + seed produce identical results?

```bash
cargo run -- --config configs/scenarios/baseline.toml --seed 123
cargo run -- --config configs/scenarios/baseline.toml --seed 123
```

**Analysis:** Both runs produce exactly identical output. This is also validated by the `deterministic_replay` integration test in `tests/smoke.rs`, which asserts that message counts and detection counts match across two runs with the same seed. Determinism is guaranteed because all randomness flows through a single `StdRng` seeded at startup.

---

### Summary of Results

| Scenario | Strategy | Detections | FP Rate | Latency | Key Finding |
|---|---|---|---|---|---|
| Stable baseline | Fixed-timeout | 0 | 0.00 | N/A | Correctly silent |
| Stable baseline | Adaptive | 0 | 0.00 | N/A | Correctly silent |
| Stable baseline | Gossip | 0 | 0.00 | N/A | Correctly silent |
| Crash + recovery | Fixed-timeout | 19 | 0.53 | 171 ticks | Detects crash; recovery causes transient FPs |
| Crash + recovery | Adaptive | 19 | 0.53 | 171 ticks | Matches fixed-timeout in low-jitter |
| Crash + recovery | Gossip (t=3) | 10 | 0.10 | 260 ticks | Best FP rate; slower detection due to gossip propagation |
| High jitter | Fixed-timeout | 10 | 1.00 | N/A | Jitter causes false positives |
| High jitter | Adaptive | 10 | 1.00 | N/A | Similar FP rate — needs tuning |
| 5% packet loss | Fixed-timeout | 21 | 1.00 | N/A | Consecutive drops trigger FPs |
| 15% packet loss | Fixed-timeout | 170 | 1.00 | N/A | Severe FP rate under heavy loss |
| Partition (3000 ticks) | Fixed-timeout | 50 | 1.00 | N/A | All cross-group nodes suspected |

## Simulation Startup

When the scenario builder creates an engine:

1. Nodes are created with IDs 1 through N, each with a peers list of all other nodes
2. A failure detector is instantiated for each node based on the chosen strategy
3. Initial `HeartbeatSend` events are scheduled at random offsets within the first heartbeat interval (staggered to avoid artificial synchronization)
4. Initial `DetectorTick` events are scheduled **one full interval later** than heartbeats, giving heartbeats time to arrive before the first detection check runs
5. Fault injection events from the `[[faults]]` config are scheduled at their specified ticks

This warmup design prevents false positives at simulation start.
