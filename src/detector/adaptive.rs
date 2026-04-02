use std::collections::HashMap;

use crate::clock::Tick;
use crate::detector::FailureDetector;
use crate::node::NodeId;

/// Failure detector that dynamically adjusts its timeout based on observed
/// heartbeat inter-arrival times using an exponentially weighted moving average.
///
/// Inspired by the Phi Accrual detector and TCP RTT estimation.
#[derive(Debug)]
pub struct AdaptiveDetector {
    /// EWMA smoothing factor (0 < alpha <= 1). Higher values weight recent samples more.
    alpha: f64,
    /// Safety multiplier applied to the estimated timeout.
    safety_multiplier: f64,
    /// Per-node EWMA of inter-arrival time.
    ewma: HashMap<NodeId, f64>,
    /// Per-node tick of last heartbeat received.
    last_heartbeat: HashMap<NodeId, Tick>,
    /// Nodes being monitored.
    monitored: Vec<NodeId>,
}

impl AdaptiveDetector {
    pub fn new(alpha: f64, safety_multiplier: f64, monitored: Vec<NodeId>) -> Self {
        Self {
            alpha,
            safety_multiplier,
            ewma: HashMap::new(),
            last_heartbeat: HashMap::new(),
            monitored,
        }
    }

    // TODO: Implement adaptive timeout computation
    // - On each heartbeat: compute inter-arrival delta, update EWMA
    // - Suspected if (current_tick - last_heartbeat) > ewma * safety_multiplier
}

impl FailureDetector for AdaptiveDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        if let Some(&prev) = self.last_heartbeat.get(&from) {
            let delta = (tick - prev) as f64;
            let current = self.ewma.get(&from).copied().unwrap_or(delta);
            let updated = self.alpha * delta + (1.0 - self.alpha) * current;
            self.ewma.insert(from, updated);
        }
        self.last_heartbeat.insert(from, tick);
    }

    fn on_tick(&mut self, _tick: Tick) {
        // TODO: Implement suspicion check using adaptive thresholds
    }

    fn suspected_nodes(&self) -> Vec<NodeId> {
        // TODO: Compare (current_tick - last_heartbeat) against ewma * safety_multiplier
        let _ = (self.safety_multiplier, &self.monitored);
        Vec::new()
    }
}
