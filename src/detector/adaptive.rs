use std::any::Any;
use std::collections::HashMap;

use crate::clock::Tick;
use crate::detector::FailureDetector;
use crate::node::NodeId;

/// Default EWMA value used before any heartbeats are observed.
const DEFAULT_EWMA: f64 = 100.0;

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
    /// Current simulation tick, updated via on_tick.
    current_tick: Tick,
}

impl AdaptiveDetector {
    pub fn new(alpha: f64, safety_multiplier: f64, monitored: Vec<NodeId>) -> Self {
        Self {
            alpha,
            safety_multiplier,
            ewma: HashMap::new(),
            last_heartbeat: HashMap::new(),
            monitored,
            current_tick: 0,
        }
    }
}

impl FailureDetector for AdaptiveDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        if let Some(&prev) = self.last_heartbeat.get(&from) {
            let delta = (tick - prev) as f64;
            // Seed the EWMA with the first real inter-arrival on first update;
            // after that, apply the standard EWMA update.
            let current = self.ewma.get(&from).copied().unwrap_or(delta);
            let updated = self.alpha * delta + (1.0 - self.alpha) * current;
            self.ewma.insert(from, updated);
        }
        self.last_heartbeat.insert(from, tick);
    }

    fn on_tick(&mut self, tick: Tick) {
        self.current_tick = tick;
    }

    fn suspected_nodes(&self) -> Vec<NodeId> {
        if self.current_tick == 0 {
            return Vec::new();
        }
        self.monitored
            .iter()
            .filter(|&&node| {
                let threshold =
                    self.ewma.get(&node).copied().unwrap_or(DEFAULT_EWMA) * self.safety_multiplier;
                match self.last_heartbeat.get(&node) {
                    Some(&last) => (self.current_tick - last) as f64 > threshold,
                    None => true,
                }
            })
            .copied()
            .collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapts_to_heartbeat_interval() {
        let mut det = AdaptiveDetector::new(0.5, 2.0, vec![1]);
        // Simulate regular heartbeats every 50 ticks
        for t in (50..=500).step_by(50) {
            det.on_heartbeat(1, t);
        }
        det.on_tick(510);
        // EWMA should be ~50, threshold = 50*2 = 100. Gap is 10, not suspected.
        assert!(det.suspected_nodes().is_empty());

        det.on_tick(700);
        // Gap is 200 > threshold ~100, should be suspected.
        assert_eq!(det.suspected_nodes(), vec![1]);
    }

    #[test]
    fn no_heartbeat_means_suspected() {
        let mut det = AdaptiveDetector::new(0.5, 2.0, vec![1, 2]);
        det.on_tick(200);
        assert_eq!(det.suspected_nodes().len(), 2);
    }
}
