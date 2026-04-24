use crate::clock::Tick;

/// Unique identifier for a node in the cluster.
pub type NodeId = u64;

/// Possible states of a simulated node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// Node is running and sending heartbeats.
    Alive,
    /// Node has crashed and is not sending messages.
    Crashed,
}

/// A simulated cluster node.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub state: NodeState,
    /// Ticks between heartbeat sends.
    pub heartbeat_interval: Tick,
    /// Ticks between detector checks.
    pub detector_interval: Tick,
    /// IDs of all other nodes in the cluster.
    pub peers: Vec<NodeId>,
}

impl Node {
    /// Convenience constructor with default intervals and no peers.
    /// Primarily used in unit tests; production code uses [`Node::with_config`].
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            state: NodeState::Alive,
            heartbeat_interval: 100,
            detector_interval: 100,
            peers: Vec::new(),
        }
    }

    pub fn with_config(
        id: NodeId,
        heartbeat_interval: Tick,
        detector_interval: Tick,
        peers: Vec<NodeId>,
    ) -> Self {
        Self {
            id,
            state: NodeState::Alive,
            heartbeat_interval,
            detector_interval,
            peers,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.state == NodeState::Alive
    }

    pub fn crash(&mut self) {
        self.state = NodeState::Crashed;
    }

    pub fn recover(&mut self) {
        self.state = NodeState::Alive;
    }
}
