# Design Document

## Overview

`faultsim` is a discrete-event simulator written in Rust. It models a cluster of nodes that exchange heartbeat messages over a simulated network with configurable latency, jitter, and failure characteristics. Pluggable failure-detector modules observe message patterns and make liveness decisions.

## Architecture

```
┌─────────────────────────────────────────────┐
│                  Scenario                    │
│  (loads config, wires components, collects)  │
├─────────────────────────────────────────────┤
│              Simulation Engine               │
│         (drives the event loop)              │
├──────────┬──────────┬───────────────────────┤
│  Clock   │  Event   │     Metrics           │
│          │  Queue   │     Collector          │
├──────────┴──────────┴───────────────────────┤
│                  Network                     │
│  (delay model, jitter, partitions, drops)    │
├─────────────────────────────────────────────┤
│          Nodes + Failure Detectors           │
│  (heartbeat sender/receiver, detector logic) │
└─────────────────────────────────────────────┘
```

## Core Components

### Clock (`clock.rs`)

A logical simulation clock that advances in discrete ticks. All time references in the simulator use `Tick` (a `u64` alias). The clock does not track wall-clock time — it advances only when the engine processes events.

### Event Queue (`event.rs`)

A priority queue (min-heap by scheduled tick) of `Event` values. Each event carries a target tick, a source, and a payload describing what should happen (message arrival, timeout firing, node crash, etc.).

### Network (`network.rs`)

Models message delivery between nodes. Accepts a sent message and returns the delivery tick based on:
- Base latency
- Jitter (sampled from a configurable distribution)
- Drop probability
- Partition rules (which node pairs can communicate)

### Nodes (`node.rs`)

Each node has an ID, a state (alive, crashed, suspected), and a reference to its failure detector. Nodes periodically send heartbeats and process incoming messages.

### Failure Detectors (`detector/`)

A trait `FailureDetector` defines the interface:

```rust
pub trait FailureDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick);
    fn on_tick(&mut self, tick: Tick);
    fn suspected_nodes(&self) -> Vec<NodeId>;
}
```

Three implementations:
- `FixedTimeoutDetector` — suspects a node if no heartbeat arrives within a fixed window
- `AdaptiveDetector` — maintains an EWMA of inter-arrival times and adjusts the threshold
- `GossipDetector` — exchanges suspicion lists with peers and aggregates evidence

### Metrics (`metrics.rs`)

Collects events during simulation:
- Each detection event (true positive, false positive, detection latency)
- Message counts per tick
- State transitions

Provides summary statistics after the run completes.

### Scenario (`scenario.rs`)

Reads a TOML configuration file and assembles the simulation: creates nodes, configures the network, selects the detector strategy, schedules fault-injection events, and runs the engine.

### Configuration (`config.rs`)

Deserialization types for scenario TOML files. Covers cluster size, network parameters, detector choice, fault schedule, and run duration.

## Design Principles

1. **Deterministic replay** — given the same config and RNG seed, a simulation produces identical results
2. **Pluggable detectors** — new strategies implement the `FailureDetector` trait with no engine changes
3. **Separation of concerns** — the network model, detector logic, and metrics collection are independent
4. **Minimal dependencies** — only `serde`, `toml`, `rand`, and `clap`
5. **Research-friendly** — configs are human-readable TOML; outputs are structured for analysis
