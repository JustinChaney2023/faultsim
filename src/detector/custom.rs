//! Custom failure-detection strategy — edit this file to implement your algorithm.
//!
//! # Quick-start
//!
//! 1. Implement your logic in the three `FailureDetector` methods below.
//! 2. Set `strategy = "custom"` in your scenario TOML.
//! 3. Pass named parameters via `[detector.params]` (all values are f64):
//!
//!    ```toml
//!    [detector]
//!    strategy = "custom"
//!    [detector.params]
//!    safety_factor = 1.5
//!    window        = 20.0
//!    ```
//!
//! 4. Run: `cargo run -- run --config configs/scenarios/custom_example.toml`
//!
//! The simulator calls your detector like this on every node:
//!
//!   ┌─ HeartbeatSend ──► [network] ──► HeartbeatArrival ──► on_heartbeat(from, tick)
//!   │
//!   └─ DetectorTick (every heartbeat_interval ticks)
//!         on_tick(tick)
//!         suspected_nodes() ──► engine records detections
//!
//! # Default implementation: Windowed-Max detector
//!
//! The detector below is a fully-working example, not a stub. It keeps a
//! sliding window of the last N inter-arrival times and suspects a node when
//! the current silence exceeds `safety_factor × max(window)`. Unlike
//! FixedTimeout, it adapts to the observed network: on a clean 10-jitter
//! network the threshold self-tunes to ~12 ticks; on a 38-jitter network it
//! automatically widens to ~138 ticks, avoiding the false positives that
//! FixedTimeout produces in `high_jitter.toml`.
//!
//! Replace this implementation with your own algorithm. The only requirement
//! is that the three trait methods compile and return sensible values.

use std::any::Any;
use std::collections::{HashMap, VecDeque};

use crate::clock::Tick;
use crate::detector::FailureDetector;
use crate::node::NodeId;

// ── State ─────────────────────────────────────────────────────────────────────

pub struct CustomDetector {
    // ── Your parameters (read from [detector.params] in the TOML) ────────────

    /// Timeout = safety_factor × max inter-arrival in the window.
    safety_factor: f64,
    /// Number of inter-arrival samples to keep per monitored node.
    window_size: usize,

    // ── Internal state (feel free to add or remove fields) ───────────────────

    /// Nodes this detector is responsible for monitoring.
    monitored: Vec<NodeId>,
    /// Per-node sliding window of inter-arrival times.
    intervals: HashMap<NodeId, VecDeque<u64>>,
    /// Per-node tick of last heartbeat received.
    last_heartbeat: HashMap<NodeId, Tick>,
    /// Current simulation tick, updated by on_tick.
    current_tick: Tick,
}

impl CustomDetector {
    /// Called by the engine during scenario setup.
    /// `params` contains every key from `[detector.params]` in the TOML.
    /// `monitored` is the list of peer node IDs this instance watches.
    pub fn new(params: &HashMap<String, f64>, monitored: Vec<NodeId>) -> Self {
        let safety_factor = *params.get("safety_factor").unwrap_or(&2.0);
        let window_size = *params.get("window").unwrap_or(&10.0) as usize;
        Self {
            safety_factor,
            window_size: window_size.max(1),
            monitored,
            intervals: HashMap::new(),
            last_heartbeat: HashMap::new(),
            current_tick: 0,
        }
    }

    /// Compute the current dynamic timeout for a node based on its window.
    /// Returns None if there are not yet enough samples to estimate.
    fn timeout_for(&self, node: NodeId) -> Option<u64> {
        let window = self.intervals.get(&node)?;
        if window.is_empty() {
            return None;
        }
        let max_interval = *window.iter().max().unwrap() as f64;
        Some((max_interval * self.safety_factor).ceil() as u64)
    }
}

// ── FailureDetector implementation ────────────────────────────────────────────

impl FailureDetector for CustomDetector {
    /// Called every time a heartbeat from `from` arrives at this node.
    /// Update your inter-arrival history, EWMA, or any other state here.
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        if let Some(&prev) = self.last_heartbeat.get(&from) {
            let interval = tick - prev;
            let window = self.intervals.entry(from).or_default();
            window.push_back(interval);
            while window.len() > self.window_size {
                window.pop_front();
            }
        }
        self.last_heartbeat.insert(from, tick);
    }

    /// Called once per heartbeat interval on every live node.
    /// Update time-based state here (e.g. advance an EWMA, check timers).
    fn on_tick(&mut self, tick: Tick) {
        self.current_tick = tick;
    }

    /// Return the set of nodes you currently believe are failed.
    /// The engine deduplicates consecutive suspicions for the same node, so
    /// it is safe to return the same node repeatedly while it is suspected.
    fn suspected_nodes(&self) -> Vec<NodeId> {
        self.monitored
            .iter()
            .copied()
            .filter(|&node| {
                // If we have never heard from this node, don't suspect yet
                // (detector is still warming up).
                let Some(&last) = self.last_heartbeat.get(&node) else {
                    return false;
                };
                let Some(timeout) = self.timeout_for(node) else {
                    return false;
                };
                self.current_tick.saturating_sub(last) > timeout
            })
            .collect()
    }

    // ── Leave these as-is ─────────────────────────────────────────────────────

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    // Optional: expose a continuous suspicion value for φ-timeline logging.
    // Return None (default) if your algorithm doesn't produce one.
    // fn phi_for_node(&self, node: NodeId) -> Option<f64> { None }
}
