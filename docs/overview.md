# faultsim — Project Overview & Briefing

A self-contained brief on what this project is, what we've built, and what the results mean. Written so you can walk into a meeting and speak fluently about every component.

---

## 1. The one-paragraph version

`faultsim` is a discrete-event simulator (in Rust) for studying **how failure detectors misbehave under unstable network conditions**. Concretely: when a healthy node gets falsely declared dead because of jitter, packet drops, or partitions, that's *misclassification*, and it cascades into bad load-balancing decisions, leader-election thrashing, and unnecessary failovers in real systems. We compare five detection strategies — three textbook approaches (fixed-timeout, EWMA-adaptive, gossip) and two state-of-the-art algorithms from the research literature (φ-accrual 2004, Satzger 2007) — across reproducible scenarios with controlled fault injection. Output: false-positive rates, detection latencies, and messaging overhead per (detector, scenario) cell. Goal: a journal paper that empirically characterizes detector behavior across the conditions where each strategy starts to break.

## 2. The research question

> Under what network conditions do common failure-detection strategies misclassify healthy nodes as failed, and how do adaptive or gossip-assisted approaches compare to fixed-timeout methods?

The interesting word is *misclassify*. There is no detector that can be both arbitrarily fast and arbitrarily accurate — that's a hard theoretical result. The interesting empirical question is *where* on the latency-vs-precision frontier each strategy lives, and *how* that position degrades as the network gets worse.

## 3. Why this matters (the elevator pitch for a mentor or reviewer)

Production failure detectors get tuned by feel. Cassandra ships with φ-accrual at threshold 8. Akka uses the same. HashiCorp's Consul uses SWIM. None of these defaults are derived from a controlled comparison — they're choices that worked well enough for the systems they were built for. A simulation harness that runs all five algorithms against identical, reproducible adverse conditions is the kind of thing that lets you say "use detector X if your network looks like Y" with evidence behind it. That's the framing for a measurements / distributed-computing journal submission.

---

## 4. How the simulator works (architecture)

Three things to know:

**1. Discrete-event, not real-time.** Time is a `u64` tick counter. The simulator advances by pulling the next event off a min-heap priority queue, jumping the clock to that event's tick, and dispatching to a handler. Nine event types cover the entire vocabulary: heartbeat send/arrive, gossip round/arrive, node crash/recover, partition start/end, detector tick. **Implication:** given the same config and RNG seed, a run is bit-for-bit reproducible. Same simulator can be re-run by a reviewer years later and produce identical numbers.

**2. The detector is pluggable.** A `FailureDetector` trait with three methods (`on_heartbeat`, `on_tick`, `suspected_nodes`) is all a strategy needs to implement. Each node has its own detector instance. The engine doesn't know whether a detector is fixed-timeout or φ-accrual — it just calls the trait methods. **Implication:** adding a new SOTA baseline means writing one file, no engine changes. We added two today.

**3. Scenarios are TOML.** `[simulation]`, `[cluster]`, `[network]`, `[detector]`, and a `[[faults]]` schedule. Every experimental result is one config file plus a seed. Every config file lives in `configs/scenarios/` and is committed. This is the reproducibility story we tell reviewers.

**File map:**
- `src/engine.rs` — the event loop, dispatch table, suspicion bookkeeping.
- `src/detector/` — five detector implementations, all behind the same trait.
- `src/network.rs` — latency + jitter + drop probability + partition rules.
- `src/scenario.rs` — wires a TOML file into a running engine.
- `src/metrics.rs` — records detections, computes FP rate and detection latency, exports CSV.

---

## 5. The five detectors (be able to explain each)

### 5.1 Fixed-timeout heartbeat
**Mechanism:** if no heartbeat from node N has arrived in the last T ticks, suspect N.
**Knobs:** the timeout T.
**Strengths:** trivial, one-line implementation, predictable behavior on a stable network.
**Weaknesses:** zero adaptivity. If the network slows down, you start producing false positives. If the network speeds up, you waste detection latency.

### 5.2 EWMA-adaptive
**Mechanism:** maintain an exponentially-weighted moving average of inter-arrival times. Suspect when the current gap exceeds `EWMA × safety_multiplier`.
**Knobs:** the smoothing factor α (how reactive vs. how stable), the safety multiplier (typically 2–4×).
**Strengths:** adapts to a slowly-changing network. Low memory.
**Weaknesses:** it's a mean estimator; it has no notion of distribution. A bursty network with a high tail will produce false positives even though the mean is fine. Conceptually a poor cousin of φ-accrual — the codebase comment honestly calls it "inspired by" φ-accrual rather than being it.

### 5.3 Gossip-assisted
**Mechanism:** every node runs a local fixed-timeout detector, but a node is only "really" suspected once K independent peers have confirmed the suspicion via periodic gossip. Local suspicions are gossiped to a random fanout of peers each round.
**Knobs:** local timeout, suspicion threshold K (we use 3), gossip interval, fanout.
**Strengths:** dramatically lowers false-positive rate by requiring corroboration. Robust to one detector having a bad view.
**Weaknesses:** higher detection latency (you have to wait for K peers to agree), higher messaging overhead. A genuinely partitioned node may never accumulate enough confirmations.
**Note:** this is *not* SWIM. SWIM uses indirect *probing* (asking peers to ping a node on your behalf) — a different mechanism. We use gossip of suspicion lists.

### 5.4 φ-accrual (Hayashibara, Defago, Yared, Katayama — SRDS 2004)
**The canonical SOTA accrual detector.** Used as the reference baseline in Cassandra and Akka.

**Mechanism:** maintain a sliding window of inter-arrival times. When queried at tick *t* with last heartbeat at tick *t_last*:
1. Δ = t − t_last.
2. Fit a Normal(μ, σ) to the window.
3. P_later = 1 − Φ((Δ − μ) / σ) where Φ is the standard normal CDF — the probability that under the model, no heartbeat would have arrived yet.
4. φ = −log₁₀(P_later). Suspect if φ > threshold.

**Knobs:** threshold θ (8 = aggressive, 12 = moderate, 16 = conservative — values from the paper, also Cassandra's defaults), window size, minimum stddev floor.

**Why "accrual":** the suspicion level *accrues* continuously rather than being a binary up/down decision. φ grows smoothly as a heartbeat becomes more overdue, so an application can choose its own threshold per-call (e.g., a leader election might use a stricter threshold than a load-balancer health check).

**Strengths:** principled probabilistic interpretation; threshold has clean semantics ("φ=8 means probability of false positive at this moment is ≤ 10⁻⁸ under the Normal model"); robust to moderate jitter because the σ in the model widens as observed jitter widens.

**Weaknesses:** assumes inter-arrival times are Normal. If they aren't (heavy-tailed, bimodal), the model is wrong and the threshold semantics break.

**Implementation note:** to avoid pulling in a dependency for `erf`, we implemented Abramowitz & Stegun's polynomial approximation (max error 1.5×10⁻⁷) inline. Same dependency footprint as before — `serde`, `toml`, `rand`, `clap`.

### 5.5 Adaptive accrual (Satzger, Pietzowski, Trumler, Ungerer — SAC 2007)
**A 2007 improvement on φ-accrual.** Same suspicion formula (φ = −log₁₀(P_later)), but **drops the Normal-distribution assumption** — uses the *empirical CDF* directly. P_later is just the fraction of historical inter-arrival times that were ≥ Δ.

**Mechanism:** sliding window, no parametric fit. P_later(Δ) = |{s ∈ window : s ≥ Δ}| / |window|.

**Knobs:** threshold θ (recommended values are smaller than φ-accrual's, typically 1–4), window size n.

**Strengths:** zero distributional assumptions. Should outperform φ-accrual when the inter-arrival distribution isn't Normal — heavy-tailed, bimodal, anything where a Gaussian fit lies.

**Weaknesses:** **hard ceiling.** φ inside the observed range is bounded by log₁₀(n). With n=100, the maximum φ from interpolation is 2.0. Past that, P_later drops to 0 and φ saturates to infinity. So thresholds at or above log₁₀(n) effectively reduce to "fire whenever Δ exceeds the largest observed inter-arrival." This is a structural limitation and a reviewer-bait gotcha — anyone reporting Satzger results without naming (n, θ) and the ceiling effect is hiding the ball.

---

## 6. What we built today (2026-04-15)

Two SOTA detectors implemented end-to-end: φ-accrual and Satzger 2007. Each is a single file in `src/detector/`, wired into the `DetectorStrategy` enum in `src/config.rs` and the engine builder in `src/scenario.rs`. Each ships with:
- A scenario config matching the existing crash scenarios (apples-to-apples comparison).
- A scenario config matching the existing high-jitter scenario.
- Six unit tests covering cold start, normal behavior, saturation, edge cases.

Why these two specifically:
- **φ-accrual** is the one detector reviewers in the distributed-systems community will recognize by name. Without it, the comparison study has no anchor.
- **Satzger 2007** is the cleanest "newer" detector — same family, same API, easy head-to-head, and it sets up the experiment plan's Phase 2 work (jitter distributions). Other candidates we deliberately deferred: SWIM (older, different paradigm), Lifeguard (extends SWIM), NFD-A (extends Satzger), Rapid (consensus-based membership, very different scope).

CI is clean: 27 unit tests + 9 integration tests, fmt clean, clippy with `-D warnings` clean. We also fixed two pre-existing clippy errors in `src/network.rs` and `tests/smoke.rs` that were blocking the gate under a newer Rust toolchain.

---

## 7. Results — what we found

Single seed (42), two scenarios, five detectors. Numbers are point estimates and **not yet** confidence intervals.

### 7.1 Scenario A: crash + recovery on a clean network

10 nodes, base latency 10, jitter 2 (low), no drops. Node 3 crashes at tick 2000, recovers at 4000. Run for 10000 ticks.

| Strategy | Detection events | FP rate | Mean detection latency |
|---|---:|---:|---:|
| Fixed-timeout | 19 | 0.526 | **171 ticks** |
| EWMA-adaptive | 19 | 0.526 | **171 ticks** |
| Gossip | 10 | 0.100 | 260 ticks |
| **φ-accrual** | 19 | 0.526 | **71 ticks** |
| **Satzger 2007** | 20 | 0.550 | **71 ticks** |

**The headline:** both accrual detectors deliver **2.4× lower detection latency** than the textbook baselines at essentially identical false-positive rates. That's a clean, plottable, defensible result — and it comes from one committed config plus one seed.

**The 0.526 FP rate that shows up four times:** this is dominated by post-recovery noise, not by detector logic. When node 3 comes back at tick 4000, every other detector still has a "no recent heartbeat from 3" view until 3's first post-recovery heartbeat reaches them — and on the next detector tick they all fire. That inflates the FP count uniformly across strategies. Worth deciding with the mentor whether the experimental framing should mask this window or count it.

**Gossip wins on FP rate** (0.10 vs. 0.53) because the K=3 confirmation requirement damps spurious local suspicions. It pays ~3.7× in latency for that.

### 7.2 Scenario B: high jitter, no faults (pure FP stress test)

10 nodes, base latency 50, jitter 38 (76% of latency — extreme), no crashes. Every detection in this scenario is, by construction, a false positive.

| Strategy | Detections | FP rate |
|---|---:|---:|
| Fixed-timeout (T=200) | 10 | 1.000 — saturated |
| EWMA-adaptive (α=0.5, ×2) | 10 | 1.000 — saturated |
| Gossip | 0 | 0.000 — perfect |
| **φ-accrual (θ=8)** | 0 | 0.000 — perfect |
| **Satzger 2007 (θ=2, n=100)** | 11 | 1.000 — **regression** |

**Two findings worth presenting separately:**

1. **φ-accrual and gossip are the only detectors that survive 76% jitter cleanly.** Different mechanisms (probabilistic suspicion vs. peer confirmation), same outcome — robustness. The two textbook baselines blow up.

2. **The "newer" detector regresses, and it isn't a bug.** Satzger's empirical CDF has a structural ceiling: with n=100 samples, φ inside the observed range can't exceed log₁₀(100) = 2.0. Threshold values at or above 2 collapse to "fire when Δ exceeds the largest observed inter-arrival." Under high jitter the largest observed inter-arrival is around 176 ticks; benign delays just past that fire the detector. φ-accrual extrapolates beyond the observed range via its Normal model, so Δ has to clear roughly μ + 5.6σ ≈ 220 ticks before threshold 8 fires — comfortably above the jitter envelope.

   **The implication for the manuscript is the interesting part:** Satzger is *supposed* to win where φ-accrual loses — under non-Normal distributions (heavy-tailed, bimodal). The simulator currently models only uniform jitter. Adding Normal and Pareto in `src/network.rs` is the prerequisite for actually demonstrating Satzger's intended advantage. We have the negative result; we need the positive result to complete the story.

We tried the obvious mitigations: threshold=3 still saturates (3 > log₁₀(100)), and window_size=1000 doesn't help because the simulation only generates ~100 heartbeats per node per run (the window never fills).

---

## 8. The "make me an expert" cheat sheet

If a mentor asks…

**"Why φ-accrual specifically?"**
Because every distributed-systems reviewer has read the Hayashibara paper. Cassandra and Akka use it. It's the single detector you cannot leave out of a comparison study and be taken seriously.

**"How is your `AdaptiveDetector` different from φ-accrual?"**
It's an EWMA mean estimator with a multiplicative safety factor — no probabilistic model, no continuous suspicion level. Its doc comment honestly says "inspired by" φ-accrual. Calling it φ-accrual would be wrong, which is why we added the real thing today.

**"Why also Satzger? Aren't two accrual detectors redundant?"**
Satzger drops the Normal-distribution assumption that φ-accrual depends on. They're equivalent under Normal-like jitter (today's results confirm this), and Satzger is supposed to dominate under non-Normal jitter (we haven't tested this yet — the simulator's jitter model is uniform-only). The comparison itself is the interesting object.

**"Why didn't you implement SWIM?"**
SWIM (Das/Gupta/Motivala 2002) is *older* than φ-accrual. Different paradigm — indirect probing rather than heartbeat statistics — and a meaningful baseline to add later, especially because Lifeguard (2018) builds on it and is what HashiCorp Consul uses today. Out of scope for one session. Worth confirming with you whether the SOTA shortlist needs SWIM/Lifeguard before manuscript.

**"What about a 2023–2025 detector? Any of those?"**
We did a literature check (see §11). The 2024 output in this subfield is survey-heavy (Chaurasia, Verma & Verma 2024 in *Computer Networks* is the key reference). The closest concrete algorithm is Li & Marin 2022 (EDCC Student Forum) — an LSTM predictor with an asymmetric loss function. It is the right next implementation target, but it is not a weekend item. The honest framing for the manuscript is that ML-based detectors are the open gap the 2024 survey explicitly names, and our benchmark harness is the infrastructure you would use to evaluate one.

**"What does threshold 8 mean for φ-accrual?"**
P_later ≤ 10⁻⁸ — under the Normal model, the probability of seeing this delay is one in a hundred million. The paper recommends 8 for snappy, 12 for moderate, 16 for conservative. Cassandra ships with 8.

**"What's a typical detection latency you'd expect?"**
Bounded below by the heartbeat interval — you literally cannot detect failure faster than your own polling rate. Our heartbeat interval is 100 ticks, so anything under ~100 means "first detector tick after the crash caught it." 71 ticks (the accrual detectors) means crashes are caught within roughly one heartbeat period on average. 171 ticks (fixed/EWMA) means they wait roughly two periods.

**"Why are there 19 detection events for 1 crash?"**
Each of the 9 other nodes runs its own detector, so each independently detects the crash → 9 true positives. After the recovery at tick 4000, each of those 9 detectors briefly still has "no recent heartbeat from 3" cached → 9-10 false positives on the next detector tick. 9 + 10 ≈ 19. The recovery FPs go away once node 3's first post-recovery heartbeat reaches each detector.

**"How do you know it's deterministic?"**
A single `StdRng` is seeded from the config's `seed` and threaded through scenario building, network jitter sampling, and gossip target selection. Same seed → same numbers, every time, on any machine. We have a `deterministic_replay` test in `tests/smoke.rs` that asserts this.

**"What's the publication target?"**
Journal first — measurements or distributed-computing (Performance Evaluation, IEEE TPDS, IEEE TDSC, ACM TOCS are candidates worth narrowing down). Conference fallback is IEEE SysCon or IEEE CCECE. The journal aim is what's driving the SOTA-baseline emphasis: a reviewer at a journal will reject a comparison study that doesn't include φ-accrual.

**"What are the obvious next steps?"**
In order: (1) multi-seed runs with confidence intervals on every plot — single-seed numbers don't ship; (2) non-Normal jitter distributions in the network model so the φ-accrual-vs-Satzger comparison can actually run; (3) fill in the rest of the experiment plan (Phase 2 jitter sensitivity, Phase 3 partition behavior). All three are open items in `NEXT.md`.

---

## 9. Open questions to bring to the mentor

1. **SOTA shortlist closure.** Are these two enough, or do you also want SWIM/Lifeguard before manuscript? SWIM is the gossip-protocol baseline a distributed-systems venue will expect.
2. **Post-recovery FP framing.** The 0.53 FP rate that dominates the crash scenario is a property of the per-detector independent-view model, not of any specific algorithm. Mask the post-recovery window? Or count it and call it out?
3. **Statistical rigor bar.** How many seeds per scenario do you want for the manuscript runs? Confidence intervals as shaded regions or error bars?
4. **Non-Normal jitter.** Phase 2 of the experiment plan calls for sweeping jitter distributions (uniform, Normal, Pareto). The simulator currently models uniform only. This is the single biggest blocker for the Satzger-vs-φ-accrual story we now have a half of. Green-light implementing this next?
5. **Journal venue.** Which journal specifically? Helps fix page budget, plot conventions, and required experimental scope.

---

## 10. Repo orientation (for someone new)

- **Read first:** `README.md`, this file, `docs/design.md`, `docs/experiment-plan.md`.
- **Today's work:** `docs/logs/2026-04-15_*.md` (two log entries) and `docs/results/2026-04-15_detector-comparison.md`.
- **The detectors:** `src/detector/{fixed_timeout,adaptive,gossip,phi_accrual,adaptive_accrual}.rs`. Each is independently readable.
- **Roadmap:** `NEXT.md` at the repo root.
- **Reproduce a result:** `cargo run --release -- --config configs/scenarios/<name>.toml --output results/<name>`.
- **Reproduce *every* result from today:** the recipe is at the top of `docs/results/2026-04-15_detector-comparison.md`.

---

## 11. Positioning against recent literature (2023+)

The mentor brief ("benchmark against state-of-the-art") is only honored if we cite *current* literature, not just 2004–2007 canon. Here is what the 2023+ landscape actually contains, what we compare against, and what we deliberately do not.

### 11.1 The 2024 anchor — Chaurasia, Verma & Verma

> Bhavana Chaurasia, Anshul Verma, Pradeepika Verma. **"An in-depth and insightful exploration of failure detection in distributed systems."** *Computer Networks*, Volume 247, June 2024. DOI: [10.1016/j.comnet.2024.110432](https://doi.org/10.1016/j.comnet.2024.110432).

This is a systematic literature review — not a new algorithm — and it is the most recent peer-reviewed work that surveys the detector taxonomy we are benchmarking. Three useful things for us:

1. **Taxonomy validation.** The survey's classification of detectors (heartbeat-based, accrual, gossip/polling) maps one-to-one onto our five implementations. We are not missing a category.
2. **Identified gap — AI/ML integration.** The survey explicitly names "integrating artificial intelligence and machine learning with existing failure detection techniques in large-scale distributed systems" as a future research direction. That is also the gap in our detector lineup: everything we have implemented is statistical/threshold-based; none of it is learned.
3. **External framing for the manuscript.** Citing a 2024 survey as the state-of-the-practice reference and then positioning our work as a controlled empirical comparison *of* that taxonomy is a defensible narrative for a journal submission.

### 11.2 The concrete ML-based reference — Li & Marin

> Xiaonan Li, Olivier Marin. **"Towards Implementing ML-Based Failure Detectors."** 18th European Dependable Computing Conference (EDCC 2022), Student Forum. [arXiv:2210.00134](https://arxiv.org/abs/2210.00134).

Technically a 2022 paper (4 pages, student forum), but it is the most recent *concrete, implementable* algorithmic failure detector in the literature that is directly comparable to our existing five. It is the forward reference for the ML gap Chaurasia 2024 calls out.

**Approach.** An LSTM neural network predicts the next heartbeat arrival time from a sliding window of the η most recent inter-arrival times. A custom asymmetric loss function penalizes under-prediction (which would cause false positives) more heavily than over-prediction, tilting the detector toward conservative suspicion. A dynamic safety margin — computed from the most recent prediction errors — replaces the static threshold used by classical detectors.

**Claimed result.** ~95% prediction accuracy on real heartbeat traces, at the cost of higher computation than statistical detectors.

**Why we have not implemented it today.** Scope. An LSTM detector is a meaningfully larger lift than an accrual formula (Rust ML stack choice, training regime, determinism story for a stochastic-init model, how to separate "training" from "simulation" time in our event loop). It is the right next SOTA target, but it needs a design pass, not a weekend implementation.

**What implementing it would buy.** A concrete instance of the ML-integration direction the 2024 survey flags as an open gap. It also gives us a detector whose inductive bias is entirely empirical (LSTM learns the distribution), sitting opposite φ-accrual's strong parametric assumption — a natural third axis for the comparison study.

### 11.3 What we searched for and did not find

To be explicit: we looked for a 2023–2026 paper proposing a single, concrete failure-detection algorithm that is directly comparable to our five (same interface: heartbeat stream in, suspicion decision out). **We did not find one.** What the 2023–2025 literature contains instead:

- **Surveys and taxonomies** (Chaurasia et al. 2024; Kirti et al. 2024 in *Concurrency and Computation*). Useful framing; not benchmarkable.
- **AIOps / LLM-based failure management** (several 2024–2025 arXiv surveys on LLMs for incident detection in cloud ops). These operate at log/metric-stream scale, not heartbeat scale — they are not drop-in replacements for φ-accrual.
- **Production reliability studies** at hyperscale (Meta, ByteDance, Google TPU) documenting that timeout-based detection is strained by modern cluster scale. Motivating context; no specific algorithm to compare against.

This matters for the manuscript. The honest claim is **"we benchmark the canonical statistical detectors and identify the ML direction as the open gap, per Chaurasia et al. 2024"** — not **"we benchmark against a 2025 state-of-the-art."** There is no 2025 state-of-the-art algorithm to benchmark against in this sub-field yet. That gap is itself a citable observation.

### 11.4 Updated shortlist for mentor conversation

Re-ordered priorities given this literature check:

1. **Implement Li & Marin 2022 LSTM detector.** This is the concrete action that closes the "newer paradigm" gap the mentor's email is asking about. Promotes to P1.
2. **Non-Normal jitter distributions.** Still needed to make Satzger-vs-φ-accrual a real comparison. Remains P1.
3. **SWIM / Lifeguard.** Still on the table but lower priority than the ML detector; SWIM is *older* than φ-accrual, so it does not satisfy the "newer SOTA" criterion even though it is a different paradigm.
4. **Multi-seed runs.** Unchanged.
