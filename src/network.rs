use std::collections::HashSet;

use rand::Rng;

use crate::clock::Tick;
use crate::node::NodeId;

/// Configuration for the simulated network.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Base one-way latency in ticks.
    pub base_latency: Tick,
    /// Maximum jitter added to base latency (uniform distribution for now).
    pub jitter: Tick,
    /// Probability [0.0, 1.0] that a message is dropped.
    pub drop_probability: f64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            base_latency: 10,
            jitter: 2,
            drop_probability: 0.0,
        }
    }
}

/// Simulated network that computes delivery times and drop decisions.
#[derive(Debug)]
pub struct Network {
    pub config: NetworkConfig,
    /// Set of blocked (src, dst) pairs representing active partitions.
    /// Asymmetric: (A, B) blocks A→B but not necessarily B→A.
    blocked_pairs: HashSet<(NodeId, NodeId)>,
}

impl Network {
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            blocked_pairs: HashSet::new(),
        }
    }

    /// Apply a partition: nodes in different groups cannot communicate.
    /// Each group is a Vec of NodeIds that can talk to each other.
    pub fn apply_partition(&mut self, groups: &[Vec<NodeId>]) {
        // Block all cross-group pairs (both directions for symmetric partitions).
        for (i, group_a) in groups.iter().enumerate() {
            for group_b in groups.iter().skip(i + 1) {
                for &a in group_a {
                    for &b in group_b {
                        self.blocked_pairs.insert((a, b));
                        self.blocked_pairs.insert((b, a));
                    }
                }
            }
        }
    }

    /// Remove all active partition rules.
    pub fn clear_partitions(&mut self) {
        self.blocked_pairs.clear();
    }

    /// Compute the delivery tick for a message sent at `send_tick`.
    /// Returns `None` if the message is dropped or blocked by a partition.
    pub fn delivery_tick<R: Rng>(
        &self,
        from: NodeId,
        to: NodeId,
        send_tick: Tick,
        rng: &mut R,
    ) -> Option<Tick> {
        // Check partition rules.
        if self.blocked_pairs.contains(&(from, to)) {
            return None;
        }

        // Check for drop.
        if self.config.drop_probability > 0.0 && rng.gen::<f64>() < self.config.drop_probability {
            return None;
        }

        let jitter = if self.config.jitter > 0 {
            rng.gen_range(0..=self.config.jitter)
        } else {
            0
        };

        Some(send_tick + self.config.base_latency + jitter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn no_drop_delivers_within_bounds() {
        let net = Network::new(NetworkConfig {
            base_latency: 10,
            jitter: 5,
            drop_probability: 0.0,
        });
        let mut rng = StdRng::seed_from_u64(42);

        for _ in 0..100 {
            let tick = net.delivery_tick(1, 2, 0, &mut rng).unwrap();
            assert!((10..=15).contains(&tick), "tick {} out of bounds", tick);
        }
    }

    #[test]
    fn partition_blocks_messages() {
        let mut net = Network::new(NetworkConfig::default());
        let mut rng = StdRng::seed_from_u64(0);

        // Before partition, messages deliver.
        assert!(net.delivery_tick(1, 2, 0, &mut rng).is_some());

        // Apply partition: group [1,2] vs group [3,4].
        net.apply_partition(&[vec![1, 2], vec![3, 4]]);

        // Cross-group blocked.
        assert!(net.delivery_tick(1, 3, 0, &mut rng).is_none());
        assert!(net.delivery_tick(3, 1, 0, &mut rng).is_none());

        // Same-group still works.
        assert!(net.delivery_tick(1, 2, 0, &mut rng).is_some());

        // Clear partitions.
        net.clear_partitions();
        assert!(net.delivery_tick(1, 3, 0, &mut rng).is_some());
    }
}
