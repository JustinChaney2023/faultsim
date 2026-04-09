use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::clock::Tick;
use crate::node::NodeId;

/// The kind of event that can occur in the simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    /// A heartbeat message arrives at a node.
    HeartbeatArrival { from: NodeId, to: NodeId },
    /// A node should send its periodic heartbeat(s).
    HeartbeatSend { from: NodeId },
    /// A node crashes (injected fault).
    NodeCrash { node: NodeId },
    /// A crashed node recovers.
    NodeRecover { node: NodeId },
    /// A failure detector should run its per-tick logic.
    DetectorTick { node: NodeId },
    /// A node should initiate a gossip round — pick random peers, send suspicion list.
    GossipRound { from: NodeId },
    /// A gossip message arrives carrying a list of suspected nodes.
    GossipArrival {
        from: NodeId,
        to: NodeId,
        suspected: Vec<NodeId>,
    },
    /// A network partition begins. Nodes in different groups cannot communicate.
    PartitionStart { groups: Vec<Vec<NodeId>> },
    /// All active partitions are lifted.
    PartitionEnd,
}

/// A scheduled simulation event.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Event {
    pub tick: Tick,
    pub kind: EventKind,
}

// BinaryHeap is a max-heap; invert ordering so smallest tick is popped first.
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        other.tick.cmp(&self.tick)
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Priority queue of simulation events, ordered by tick (earliest first).
#[derive(Debug, Default)]
pub struct EventQueue {
    heap: BinaryHeap<Event>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule(&mut self, event: Event) {
        self.heap.push(event);
    }

    pub fn pop(&mut self) -> Option<Event> {
        self.heap.pop()
    }

    pub fn peek_tick(&self) -> Option<Tick> {
        self.heap.peek().map(|e| e.tick)
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_dequeue_in_tick_order() {
        let mut q = EventQueue::new();
        q.schedule(Event {
            tick: 30,
            kind: EventKind::HeartbeatSend { from: 1 },
        });
        q.schedule(Event {
            tick: 10,
            kind: EventKind::HeartbeatSend { from: 2 },
        });
        q.schedule(Event {
            tick: 20,
            kind: EventKind::HeartbeatSend { from: 3 },
        });

        assert_eq!(q.pop().unwrap().tick, 10);
        assert_eq!(q.pop().unwrap().tick, 20);
        assert_eq!(q.pop().unwrap().tick, 30);
    }
}
