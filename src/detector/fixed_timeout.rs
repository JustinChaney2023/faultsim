use std::any::Any;
use std::collections::HashMap;

use crate::clock::Tick;
use crate::detector::FailureDetector;
use crate::node::NodeId;

/// Failure detector that suspects a node if no heartbeat arrives within a fixed
/// timeout window.
#[derive(Debug)]
pub struct FixedTimeoutDetector {
    /// Maximum ticks allowed between heartbeats before suspecting a node.
    timeout: Tick,
    /// Last heartbeat tick received from each monitored node.
    last_heartbeat: HashMap<NodeId, Tick>,
    /// Set of nodes being monitored.
    monitored: Vec<NodeId>,
    /// Current simulation tick, updated via on_tick.
    current_tick: Tick,
}

impl FixedTimeoutDetector {
    pub fn new(timeout: Tick, monitored: Vec<NodeId>) -> Self {
        Self {
            timeout,
            last_heartbeat: HashMap::new(),
            monitored,
            current_tick: 0,
        }
    }
}

impl FailureDetector for FixedTimeoutDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        self.last_heartbeat.insert(from, tick);
    }

    fn on_tick(&mut self, tick: Tick) {
        self.current_tick = tick;
    }

    fn suspected_nodes(&self) -> Vec<NodeId> {
        // current_tick == 0 means the simulation just started; don't suspect yet.
        if self.current_tick == 0 {
            return Vec::new();
        }
        self.monitored
            .iter()
            .filter(|&&node| match self.last_heartbeat.get(&node) {
                Some(&last) => self.current_tick - last > self.timeout,
                // No heartbeat ever received from this node — treat as timed out.
                None => true,
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
    fn no_heartbeat_means_suspected() {
        let mut det = FixedTimeoutDetector::new(100, vec![1, 2, 3]);
        det.on_tick(200);
        let suspected = det.suspected_nodes();
        assert_eq!(suspected.len(), 3);
    }

    #[test]
    fn recent_heartbeat_not_suspected() {
        let mut det = FixedTimeoutDetector::new(100, vec![1, 2]);
        det.on_heartbeat(1, 50);
        det.on_heartbeat(2, 50);
        det.on_tick(100);
        assert!(det.suspected_nodes().is_empty());
    }

    #[test]
    fn stale_heartbeat_is_suspected() {
        let mut det = FixedTimeoutDetector::new(100, vec![1]);
        det.on_heartbeat(1, 10);
        det.on_tick(200);
        let suspected = det.suspected_nodes();
        assert_eq!(suspected, vec![1]);
    }
}
