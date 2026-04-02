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
    // TODO: Add reference to this node's failure detector instance
    // TODO: Add heartbeat interval configuration
    // TODO: Track peers list for gossip protocol
}

impl Node {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            state: NodeState::Alive,
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
