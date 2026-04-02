# 2026-04-02 — Initial Project Scaffold

## What changed

Set up the full repository structure for `faultsim` from scratch.

### Files created

**Root config:**
- `README.md` — project overview, research question, getting started
- `.gitignore` — Rust artifacts, editor files, results directory
- `.editorconfig` — consistent formatting across editors
- `rustfmt.toml` — Rust formatter settings (edition 2021, 100-char width)
- `Cargo.toml` — project metadata and dependencies (serde, toml, rand, clap)

**CI:**
- `.github/workflows/ci.yml` — GitHub Actions: fmt check, clippy, build, test on push/PR to main/dev

**Documentation:**
- `docs/proposal.md` — research proposal with motivation, question, approach, references
- `docs/design.md` — architecture overview (engine, clock, event queue, network, detectors, metrics)
- `docs/experiment-plan.md` — full experiment design with variables, phases, and analysis plan

**Simulator source (`src/`):**
- `lib.rs` — module declarations
- `main.rs` — CLI entry point using clap (config path + seed override)
- `clock.rs` — discrete tick clock with unit tests
- `event.rs` — event types + priority queue (min-heap by tick) with unit tests
- `node.rs` — node model with alive/crashed states
- `network.rs` — network model with latency, jitter, drop probability, unit tests
- `detector/mod.rs` — `FailureDetector` trait definition
- `detector/fixed_timeout.rs` — fixed-timeout heartbeat detector (scaffolded)
- `detector/adaptive.rs` — adaptive EWMA-based detector (scaffolded)
- `detector/gossip.rs` — gossip-assisted suspicion detector (scaffolded)
- `engine.rs` — simulation engine with event dispatch loop, unit tests
- `metrics.rs` — metrics collector (detections, crashes, recoveries, message counts)
- `config.rs` — TOML config deserialization types
- `scenario.rs` — config loader

**Supporting:**
- `configs/scenarios/baseline.toml` — baseline scenario (10 nodes, clean network, fixed-timeout)
- `scripts/run_experiment.sh` — batch experiment runner shell script
- `results/.gitkeep` — placeholder for git-ignored results directory
- `tests/smoke.rs` — 6 integration tests (clock, events, nodes, network, engine, config parsing)

## Decisions and reasoning

- **Rust** chosen for performance and correctness — important for a simulator that may run millions of events.
- **Discrete-event simulation** rather than real-time — gives deterministic replay (same seed = same results), which is critical for reproducible research.
- **Pluggable detector trait** — adding a new failure-detection strategy means implementing one trait, no engine changes. This keeps the research extensible.
- **TOML configs** — human-readable, diffable, easy to version-control alongside code.
- **Minimal dependencies** (serde, toml, rand, clap) — avoids framework lock-in and keeps compile times reasonable.
- **`EventQueue::pop()` instead of `next()`** — clippy flagged `next()` as confusable with `Iterator::next`. Renamed early to keep the codebase lint-clean from day one.
- **Three detector strategies** (fixed-timeout, adaptive, gossip) scaffolded as stubs with TODO markers — enough structure to guide implementation without premature complexity.

## Status

- `cargo build` — clean (zero warnings)
- `cargo clippy -- -D warnings` — clean
- `cargo fmt -- --check` — clean
- `cargo test` — 11 tests pass (5 unit + 6 integration)

## Memory

This is the starting point. Everything compiles and passes CI checks, but the simulator doesn't actually *do* anything yet — the engine dispatch is stubbed, detectors return empty suspicion lists, and main just prints a placeholder message. The real work starts with wiring the engine to nodes and detectors (issue #1 in the suggested plan).
