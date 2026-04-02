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
    // TODO: Add partition rules (set of blocked (src, dst) pairs)
    // TODO: Support asymmetric partitions
    // TODO: Support configurable jitter distributions (normal, Pareto)
}

impl Network {
    pub fn new(config: NetworkConfig) -> Self {
        Self { config }
    }

    /// Compute the delivery tick for a message sent at `send_tick`.
    /// Returns `None` if the message is dropped.
    pub fn delivery_tick<R: Rng>(
        &self,
        _from: NodeId,
        _to: NodeId,
        send_tick: Tick,
        rng: &mut R,
    ) -> Option<Tick> {
        // Check for drop
        if self.config.drop_probability > 0.0 && rng.gen::<f64>() < self.config.drop_probability {
            return None;
        }

        // TODO: Check partition rules

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
            assert!(tick >= 10 && tick <= 15, "tick {} out of bounds", tick);
        }
    }
}
