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
}

impl FixedTimeoutDetector {
    pub fn new(timeout: Tick, monitored: Vec<NodeId>) -> Self {
        Self {
            timeout,
            last_heartbeat: HashMap::new(),
            monitored,
        }
    }
}

impl FailureDetector for FixedTimeoutDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        self.last_heartbeat.insert(from, tick);
    }

    fn on_tick(&mut self, _tick: Tick) {
        // Suspicion is computed on query — no per-tick work needed for fixed timeout.
    }

    fn suspected_nodes(&self) -> Vec<NodeId> {
        // TODO: The engine must pass the current tick to make this work properly.
        // For now, this is a placeholder that returns an empty list.
        // Once the engine integration is done, compare (current_tick - last_heartbeat) > timeout.
        let _ = (self.timeout, &self.last_heartbeat, &self.monitored);
        Vec::new()
    }
}
