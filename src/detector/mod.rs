pub mod adaptive;
pub mod fixed_timeout;
pub mod gossip;

use crate::clock::Tick;
use crate::node::NodeId;

/// Trait that all failure-detection strategies must implement.
///
/// The simulation engine calls `on_heartbeat` when a heartbeat message arrives
/// and `on_tick` on every detector-tick event. The engine queries `suspected_nodes`
/// to determine which nodes the detector currently considers failed.
pub trait FailureDetector {
    /// Called when a heartbeat from `from` arrives at the current tick.
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick);

    /// Called periodically to let the detector update internal state.
    fn on_tick(&mut self, tick: Tick);

    /// Returns the set of nodes currently suspected as failed.
    fn suspected_nodes(&self) -> Vec<NodeId>;
}
