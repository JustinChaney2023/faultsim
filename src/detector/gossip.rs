use std::collections::HashMap;

use crate::clock::Tick;
use crate::detector::FailureDetector;
use crate::node::NodeId;

/// Failure detector that combines local heartbeat monitoring with
/// gossip-disseminated suspicion.
///
/// A node is suspected only when multiple peers independently report missed
/// heartbeats. Inspired by SWIM and Lifeguard protocols.
#[derive(Debug)]
pub struct GossipDetector {
    /// Number of independent suspicions required before declaring failure.
    suspicion_threshold: u32,
    /// Per-node suspicion count from distinct sources.
    suspicion_counts: HashMap<NodeId, u32>,
    /// Last heartbeat tick received from each monitored node.
    last_heartbeat: HashMap<NodeId, Tick>,
    /// Nodes being monitored.
    #[allow(dead_code)]
    monitored: Vec<NodeId>,
    // TODO: Add gossip interval and peer sampling configuration
    // TODO: Track which peers have reported suspicion (avoid double-counting)
    // TODO: Implement suspicion decay over time
}

impl GossipDetector {
    pub fn new(suspicion_threshold: u32, monitored: Vec<NodeId>) -> Self {
        Self {
            suspicion_threshold,
            suspicion_counts: HashMap::new(),
            last_heartbeat: HashMap::new(),
            monitored,
        }
    }

    // TODO: Implement gossip round logic
    // - Periodically select random peers and exchange suspicion lists
    // - Merge incoming suspicion evidence
    // - Increment counts when new independent evidence arrives

    // TODO: Implement suspicion decay
    // - Reduce suspicion counts over time if no new evidence arrives
}

impl FailureDetector for GossipDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        self.last_heartbeat.insert(from, tick);
        // Receiving a direct heartbeat clears suspicion.
        self.suspicion_counts.remove(&from);
    }

    fn on_tick(&mut self, _tick: Tick) {
        // TODO: Check for locally missed heartbeats and increment local suspicion
        // TODO: Trigger gossip rounds at configured intervals
    }

    fn suspected_nodes(&self) -> Vec<NodeId> {
        self.suspicion_counts
            .iter()
            .filter(|(_, &count)| count >= self.suspicion_threshold)
            .map(|(&node, _)| node)
            .collect()
    }
}
