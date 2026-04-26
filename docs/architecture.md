# Architecture

`faultsim` is a single-threaded discrete-event simulator. One event loop, one clock, one RNG. All variability lives in pluggable detector strategies and the network model. These ASCII diagrams capture the dataflow; [src/scenario.rs](../src/scenario.rs), [src/engine.rs](../src/engine.rs), and [src/detector/mod.rs](../src/detector/mod.rs) are the three files to read together.

## 1. Top-level dataflow

```
   ┌────────────────────────┐
   │  configs/scenarios/    │   cluster size, network params,
   │  *.toml                │   detector strategy, fault schedule
   └───────────┬────────────┘
               │  parsed by src/config.rs
               ▼
   ┌────────────────────────┐
   │  scenario::build_engine│   single StdRng seeded here;
   │  (src/scenario.rs)     │   threaded everywhere below
   └───────────┬────────────┘
               │
               ▼
   ┌────────────────────────────────────────────────────────┐
   │                        Engine                          │
   │                   (src/engine.rs)                      │
   │                                                        │
   │   Clock ─── EventQueue ─── Nodes ─── Detectors         │
   │                │                        │              │
   │                └──► Network ◄───────────┘              │
   │                        │                               │
   │                        ▼                               │
   │                  MetricsCollector                      │
   └───────────┬────────────────────────────────────────────┘
               │
               ▼
   ┌────────────────────────┐
   │  results/<run>/        │   detections.csv
   │                        │   summary.csv
   └────────────────────────┘
```

## 2. Event loop

```
                    ┌──────────────────────────────┐
                    │         EventQueue           │
                    │       (min-heap by tick)     │
                    └──────────────┬───────────────┘
                                   │ pop earliest
                                   ▼
                    ┌──────────────────────────────┐
                    │  event.tick > max_tick ?     │───► halt
                    └──────────────┬───────────────┘
                                   │ no
                                   ▼
                    ┌──────────────────────────────┐
                    │   Clock::advance_to(tick)    │
                    └──────────────┬───────────────┘
                                   │
                                   ▼
                    ┌──────────────────────────────┐
                    │      Engine::dispatch        │
                    │    match event.kind { … }    │
                    └──────────────┬───────────────┘
                                   │
        ┌──────────────────────────┼──────────────────────────┐
        │                          │                          │
        ▼                          ▼                          ▼
  mutate node state        invoke detector hook       schedule follow-up
  (crash / recover /       (on_heartbeat / on_tick)   events back into
   partition groups)                                  the queue
        │                          │                          │
        └──────────────────────────┴──────────────────────────┘
                                   │
                                   ▼
                       loop until queue drains
```

Periodic events (`HeartbeatSend`, `DetectorTick`, `GossipRound`) re-enqueue their next occurrence from inside their own handler — this is how the simulation sustains itself without an outer driver.

## 3. Event vocabulary

Nine `EventKind` variants in [src/event.rs](../src/event.rs). Adding new behavior almost always means adding a variant here and a handler in `Engine::dispatch`.

```
           cluster lifecycle           detection                 network faults
   ┌────────────────────────┐   ┌───────────────────┐   ┌────────────────────────┐
   │  HeartbeatSend         │   │  DetectorTick     │   │  NodeCrash             │
   │  HeartbeatArrival ─────┼──►│  (on_heartbeat/   │   │  NodeRecover           │
   │                        │   │   on_tick hooks)  │   │  PartitionStart        │
   │  GossipRound           │   │                   │   │  PartitionEnd          │
   │  GossipArrival ────────┼──►│  (gossip-only     │   │                        │
   │                        │   │   downcast path)  │   │                        │
   └────────────────────────┘   └───────────────────┘   └────────────────────────┘
```

## 4. Detector plug-in interface

Five implementations live in [src/detector/](../src/detector/); the engine only sees `Box<dyn FailureDetector>`.

```
                    ┌──────────────────────────────────────┐
                    │     trait FailureDetector            │
                    │     (src/detector/mod.rs)            │
                    │                                      │
                    │   on_heartbeat(from, tick)           │
                    │   on_tick(tick)                      │
                    │   suspected_nodes() -> Vec<NodeId>   │
                    │   as_any / as_any_mut  (downcast)    │
                    └──────────────────┬───────────────────┘
                                       │ impl
        ┌──────────────┬───────────────┼───────────────┬──────────────┐
        ▼              ▼               ▼               ▼              ▼
   ┌──────────┐  ┌──────────┐   ┌───────────┐   ┌────────────┐  ┌───────────┐
   │ Fixed-   │  │ Adaptive │   │  Gossip   │   │ PhiAccrual │  │ Adaptive- │
   │ Timeout  │  │  (EWMA)  │   │ (downcast │   │            │  │  Accrual  │
   │          │  │          │   │  path)    │   │            │  │           │
   └──────────┘  └──────────┘   └───────────┘   └────────────┘  └───────────┘
```

Gossip's `GossipRound` / `GossipArrival` paths reach gossip-specific state via `as_any_mut` rather than widening the base trait.

## 5. Network model

Every inter-node message — heartbeat or gossip — goes through exactly one call:

```
    sender  ──► Network::delivery_tick(from, to, now, rng)  ──► Option<Tick>
                                │
                ┌───────────────┼───────────────┬────────────────┐
                ▼               ▼               ▼                ▼
           base_latency      jitter         drop_prob       partition
           (constant)       (uniform         (Bernoulli)    (reject if
                            over ±jitter)                    from/to in
                                                             different
                                                             groups)
                                │
                                ▼
                     Some(now + delay)  ─►  schedule *Arrival at that tick
                     None               ─►  message is lost (no event scheduled)
```

## 6. Determinism

```
   seed (TOML or --seed flag)
          │
          ▼
     StdRng::seed_from_u64
          │
          ├─► staggered heartbeat / detector-tick / gossip-round offsets
          │
          └─► Network::delivery_tick (jitter + drop draws)
```

A single RNG instance is threaded from `scenario::build_engine` into the engine, then borrowed by the network model on every message. No `thread_rng()`. No `HashMap` iteration on observable paths. Same `(config, seed)` → bit-for-bit identical `detections.csv`, enforced by the `deterministic_replay` integration test.

## 7. Metrics pipeline

```
   detector transitions a node into suspected_nodes()
          │
          ▼
   Engine checks (detector_node, suspected_node) ∉ active_suspicions
          │                                          (dedup guard)
          ▼
   MetricsCollector::record_detection { tick, detector, suspected,
                                        true_positive, latency_from_crash }
          │
          ▼
   end-of-run CSV export  ──►  detections.csv   (one row per event)
                           └─► summary.csv      (per-run aggregate)
```

Latency is measured from the nearest preceding `NodeCrash` for the suspected node; if none, the detection is a false positive.

## 8. File map

| File | Role |
|---|---|
| [src/main.rs](../src/main.rs) | CLI entrypoint: parse args, load config, build engine, run, write CSVs |
| [src/config.rs](../src/config.rs) | TOML schema (`DetectorStrategy`, `FaultKind`, …) |
| [src/scenario.rs](../src/scenario.rs) | Config → `Engine` wiring; sole seeding site for the RNG |
| [src/engine.rs](../src/engine.rs) | Event loop, dispatch, suspicion dedup |
| [src/event.rs](../src/event.rs) | `EventKind` variants + min-heap `EventQueue` |
| [src/clock.rs](../src/clock.rs) | Monotonic tick counter |
| [src/network.rs](../src/network.rs) | `delivery_tick` — latency, jitter, drops, partitions |
| [src/node.rs](../src/node.rs) | Node state (alive / crashed) and transitions |
| [src/detector/mod.rs](../src/detector/mod.rs) | `FailureDetector` trait |
| [src/detector/{fixed_timeout,adaptive,gossip,phi_accrual,adaptive_accrual}.rs](../src/detector/) | Strategy implementations |
| [src/metrics.rs](../src/metrics.rs) | `MetricsCollector`, CSV export |
