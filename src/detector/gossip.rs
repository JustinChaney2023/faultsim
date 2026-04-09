use std::any::Any;
use std::collections::{HashMap, HashSet};

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
    /// Local timeout: ticks without a heartbeat before local suspicion fires.
    local_timeout: Tick,
    /// Per-node suspicion count from distinct sources.
    suspicion_counts: HashMap<NodeId, u32>,
    /// Track which sources have contributed suspicion for each node.
    suspicion_sources: HashMap<NodeId, HashSet<NodeId>>,
    /// Last heartbeat tick received from each monitored node.
    last_heartbeat: HashMap<NodeId, Tick>,
    /// Nodes being monitored.
    monitored: Vec<NodeId>,
    /// Current simulation tick, updated via on_tick.
    current_tick: Tick,
    /// Nodes already locally suspected (prevents double-counting per tick cycle).
    locally_suspected: HashSet<NodeId>,
    /// The ID of the node running this detector (needed to tag local suspicion source).
    owner: NodeId,
    /// Ticks between gossip rounds.
    pub gossip_interval: Tick,
    /// Number of random peers to gossip with each round.
    pub gossip_fanout: u32,
}

impl GossipDetector {
    pub fn new(
        suspicion_threshold: u32,
        local_timeout: Tick,
        monitored: Vec<NodeId>,
        owner: NodeId,
        gossip_interval: Tick,
        gossip_fanout: u32,
    ) -> Self {
        Self {
            suspicion_threshold,
            local_timeout,
            suspicion_counts: HashMap::new(),
            suspicion_sources: HashMap::new(),
            last_heartbeat: HashMap::new(),
            monitored,
            current_tick: 0,
            locally_suspected: HashSet::new(),
            owner,
            gossip_interval,
            gossip_fanout,
        }
    }

    /// Called by the engine when gossip evidence arrives from a remote peer.
    /// Only increments if this source hasn't already contributed for this node.
    pub fn on_remote_suspicion(&mut self, suspected_node: NodeId, source: NodeId) {
        let sources = self.suspicion_sources.entry(suspected_node).or_default();
        if sources.insert(source) {
            *self.suspicion_counts.entry(suspected_node).or_insert(0) += 1;
        }
    }

    /// Returns the list of nodes this detector currently locally suspects.
    /// Used by the engine to build gossip messages.
    pub fn local_suspicions(&self) -> Vec<NodeId> {
        self.locally_suspected.iter().copied().collect()
    }
}

impl FailureDetector for GossipDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        self.last_heartbeat.insert(from, tick);
        // Receiving a direct heartbeat clears all suspicion.
        self.suspicion_counts.remove(&from);
        self.suspicion_sources.remove(&from);
        self.locally_suspected.remove(&from);
    }

    fn on_tick(&mut self, tick: Tick) {
        self.current_tick = tick;

        if tick == 0 {
            return;
        }

        // Check for locally missed heartbeats and increment local suspicion.
        for &node in &self.monitored {
            let missed = match self.last_heartbeat.get(&node) {
                Some(&last) => tick - last > self.local_timeout,
                None => true,
            };

            if missed && !self.locally_suspected.contains(&node) {
                self.locally_suspected.insert(node);
                // Record as suspicion from this detector's owner.
                let sources = self.suspicion_sources.entry(node).or_default();
                if sources.insert(self.owner) {
                    *self.suspicion_counts.entry(node).or_insert(0) += 1;
                }
            }

            // If heartbeat resumes, clear local suspicion flag so it can fire again.
            if !missed {
                self.locally_suspected.remove(&node);
            }
        }
    }

    fn suspected_nodes(&self) -> Vec<NodeId> {
        self.suspicion_counts
            .iter()
            .filter(|(_, &count)| count >= self.suspicion_threshold)
            .map(|(&node, _)| node)
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
    fn requires_threshold_suspicions() {
        let mut det = GossipDetector::new(3, 100, vec![1], 99, 50, 3);
        det.on_tick(200); // local suspicion fires: count = 1 (from owner 99)
        assert!(det.suspected_nodes().is_empty());

        det.on_remote_suspicion(1, 10); // count = 2 (from peer 10)
        assert!(det.suspected_nodes().is_empty());

        det.on_remote_suspicion(1, 20); // count = 3 (from peer 20)
        assert_eq!(det.suspected_nodes(), vec![1]);
    }

    #[test]
    fn duplicate_source_not_counted() {
        let mut det = GossipDetector::new(3, 100, vec![1], 99, 50, 3);
        det.on_tick(200); // count = 1 (from 99)

        det.on_remote_suspicion(1, 10); // count = 2 (from 10)
        det.on_remote_suspicion(1, 10); // duplicate — still 2
        assert!(det.suspected_nodes().is_empty());

        det.on_remote_suspicion(1, 20); // count = 3 (from 20)
        assert_eq!(det.suspected_nodes(), vec![1]);
    }

    #[test]
    fn heartbeat_clears_suspicion() {
        let mut det = GossipDetector::new(1, 100, vec![1], 99, 50, 3);
        det.on_tick(200);
        assert_eq!(det.suspected_nodes(), vec![1]);

        det.on_heartbeat(1, 200);
        assert!(det.suspected_nodes().is_empty());
    }

    #[test]
    fn local_suspicions_exposed() {
        let mut det = GossipDetector::new(3, 100, vec![1, 2], 99, 50, 3);
        det.on_tick(200);
        let mut suspects = det.local_suspicions();
        suspects.sort();
        assert_eq!(suspects, vec![1, 2]);
    }
}
