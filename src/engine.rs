use std::collections::{HashMap, HashSet};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::clock::{Clock, Tick};
use crate::detector::gossip::GossipDetector;
use crate::detector::FailureDetector;
use crate::event::{Event, EventKind, EventQueue};
use crate::metrics::{DetectionEvent, MetricsCollector, PhiLogEntry};
use crate::network::Network;
use crate::node::{Node, NodeId};

/// The simulation engine. Drives the event loop by pulling events from the
/// queue, advancing the clock, and dispatching to the appropriate handlers.
pub struct Engine {
    pub clock: Clock,
    pub queue: EventQueue,
    pub metrics: MetricsCollector,
    max_tick: Tick,
    nodes: HashMap<NodeId, Node>,
    network: Option<Network>,
    detectors: HashMap<NodeId, Box<dyn FailureDetector>>,
    rng: Option<StdRng>,
    /// Tracks (detector_node, suspected_node) pairs to avoid duplicate detection events.
    active_suspicions: HashSet<(NodeId, NodeId)>,
    /// When true, record a φ sample from every accrual detector on each DetectorTick.
    pub phi_log_enabled: bool,
}

impl Engine {
    /// Minimal constructor for tests that don't need full simulation wiring.
    pub fn new(max_tick: Tick) -> Self {
        Self {
            clock: Clock::new(),
            queue: EventQueue::new(),
            metrics: MetricsCollector::new(),
            max_tick,
            nodes: HashMap::new(),
            network: None,
            detectors: HashMap::new(),
            rng: None,
            active_suspicions: HashSet::new(),
            phi_log_enabled: false,
        }
    }

    /// Full constructor used by the scenario builder.
    pub fn with_simulation(
        max_tick: Tick,
        nodes: HashMap<NodeId, Node>,
        network: Network,
        detectors: HashMap<NodeId, Box<dyn FailureDetector>>,
        rng: StdRng,
    ) -> Self {
        Self {
            clock: Clock::new(),
            queue: EventQueue::new(),
            metrics: MetricsCollector::new(),
            max_tick,
            nodes,
            network: Some(network),
            detectors,
            rng: Some(rng),
            active_suspicions: HashSet::new(),
            phi_log_enabled: false,
        }
    }

    /// Run the simulation until the event queue is empty or max_tick is reached.
    pub fn run(&mut self) {
        while let Some(event) = self.queue.pop() {
            if event.tick > self.max_tick {
                break;
            }

            self.clock.advance_to(event.tick);
            self.dispatch(event);
        }
    }

    fn dispatch(&mut self, event: Event) {
        match event.kind {
            EventKind::HeartbeatSend { from } => self.handle_heartbeat_send(from),
            EventKind::HeartbeatArrival { from, to } => self.handle_heartbeat_arrival(from, to),
            EventKind::NodeCrash { node } => self.handle_node_crash(node),
            EventKind::NodeRecover { node } => self.handle_node_recover(node),
            EventKind::DetectorTick { node } => self.handle_detector_tick(node),
            EventKind::GossipRound { from } => self.handle_gossip_round(from),
            EventKind::GossipArrival {
                from,
                to,
                suspected,
            } => self.handle_gossip_arrival(from, to, suspected),
            EventKind::PartitionStart { groups } => self.handle_partition_start(groups),
            EventKind::PartitionEnd => self.handle_partition_end(),
        }
    }

    fn handle_heartbeat_send(&mut self, from: NodeId) {
        let (is_alive, peers, interval) = match self.nodes.get(&from) {
            Some(node) => (node.is_alive(), node.peers.clone(), node.heartbeat_interval),
            None => return,
        };

        if !is_alive {
            return;
        }

        let tick = self.clock.now();

        // Send heartbeat to each peer via network.
        if let (Some(network), Some(rng)) = (&self.network, &mut self.rng) {
            for &peer in &peers {
                if let Some(arrival_tick) = network.delivery_tick(from, peer, tick, rng) {
                    self.queue.schedule(Event {
                        tick: arrival_tick,
                        kind: EventKind::HeartbeatArrival { from, to: peer },
                    });
                    self.metrics.record_message(tick);
                }
            }
        }

        // Re-schedule next heartbeat send.
        self.queue.schedule(Event {
            tick: tick + interval,
            kind: EventKind::HeartbeatSend { from },
        });
    }

    fn handle_heartbeat_arrival(&mut self, from: NodeId, to: NodeId) {
        // Crashed nodes can't receive messages.
        let receiver_alive = self.nodes.get(&to).is_some_and(|n| n.is_alive());
        if !receiver_alive {
            return;
        }

        let tick = self.clock.now();
        if let Some(detector) = self.detectors.get_mut(&to) {
            detector.on_heartbeat(from, tick);
        }
    }

    fn handle_node_crash(&mut self, node: NodeId) {
        if let Some(n) = self.nodes.get_mut(&node) {
            n.crash();
        }
        self.metrics.record_crash(self.clock.now(), node);
    }

    fn handle_node_recover(&mut self, node: NodeId) {
        if let Some(n) = self.nodes.get_mut(&node) {
            n.recover();
        }
        self.metrics.record_recovery(self.clock.now(), node);

        let tick = self.clock.now();

        // Clear any active suspicions targeting this node since it recovered.
        self.active_suspicions
            .retain(|&(_, suspected)| suspected != node);

        // Resume heartbeats and detector ticks.
        self.queue.schedule(Event {
            tick: tick + 1,
            kind: EventKind::HeartbeatSend { from: node },
        });
        self.queue.schedule(Event {
            tick: tick + 1,
            kind: EventKind::DetectorTick { node },
        });
    }

    fn handle_gossip_round(&mut self, from: NodeId) {
        let (is_alive, peers) = match self.nodes.get(&from) {
            Some(node) => (node.is_alive(), node.peers.clone()),
            None => return,
        };

        if !is_alive {
            return;
        }

        let tick = self.clock.now();

        // Get the local suspicion list and gossip config from the detector.
        let (local_suspicions, gossip_interval, gossip_fanout) = match self.detectors.get(&from) {
            Some(det) => match det.as_any().downcast_ref::<GossipDetector>() {
                Some(gossip) => (
                    gossip.local_suspicions(),
                    gossip.gossip_interval,
                    gossip.gossip_fanout as usize,
                ),
                None => return,
            },
            None => return,
        };

        // Pick random peers to gossip with.
        if !local_suspicions.is_empty() {
            if let Some(rng) = &mut self.rng {
                let fanout = gossip_fanout.min(peers.len());
                let mut targets = peers.clone();
                targets.shuffle(rng);
                let targets = &targets[..fanout];

                for &target in targets {
                    // Send gossip via network (subject to latency, drops, partitions).
                    if let Some(network) = &self.network {
                        if let Some(arrival_tick) = network.delivery_tick(from, target, tick, rng) {
                            self.queue.schedule(Event {
                                tick: arrival_tick,
                                kind: EventKind::GossipArrival {
                                    from,
                                    to: target,
                                    suspected: local_suspicions.clone(),
                                },
                            });
                            self.metrics.record_message(tick);
                        }
                    }
                }
            }
        }

        // Re-schedule next gossip round.
        self.queue.schedule(Event {
            tick: tick + gossip_interval,
            kind: EventKind::GossipRound { from },
        });
    }

    fn handle_gossip_arrival(&mut self, from: NodeId, to: NodeId, suspected: Vec<NodeId>) {
        // Crashed nodes can't receive messages.
        let receiver_alive = self.nodes.get(&to).is_some_and(|n| n.is_alive());
        if !receiver_alive {
            return;
        }

        if let Some(detector) = self.detectors.get_mut(&to) {
            if let Some(gossip) = detector.as_any_mut().downcast_mut::<GossipDetector>() {
                for suspected_node in suspected {
                    gossip.on_remote_suspicion(suspected_node, from);
                }
            }
        }
    }

    fn handle_partition_start(&mut self, groups: Vec<Vec<NodeId>>) {
        if let Some(network) = &mut self.network {
            network.apply_partition(&groups);
        }
    }

    fn handle_partition_end(&mut self) {
        if let Some(network) = &mut self.network {
            network.clear_partitions();
        }
    }

    fn handle_detector_tick(&mut self, node: NodeId) {
        let (is_alive, interval) = match self.nodes.get(&node) {
            Some(n) => (n.is_alive(), n.detector_interval),
            None => return,
        };

        if !is_alive {
            return;
        }

        let tick = self.clock.now();

        // Collect node IDs before the mutable borrow of detectors.
        let observed_nodes: Vec<NodeId> = self
            .nodes
            .keys()
            .copied()
            .filter(|&n| n != node)
            .collect();

        if let Some(detector) = self.detectors.get_mut(&node) {
            detector.on_tick(tick);

            // Optional φ timeline logging for accrual detectors.
            if self.phi_log_enabled {
                for &observed in &observed_nodes {
                    if let Some(phi) = detector.phi_for_node(observed) {
                        self.metrics.record_phi(PhiLogEntry {
                            tick,
                            observer: node,
                            observed,
                            phi,
                        });
                    }
                }
            }

            let suspected = detector.suspected_nodes();

            for suspected_id in suspected {
                let pair = (node, suspected_id);

                // Only record new suspicions, not re-suspicions.
                if self.active_suspicions.contains(&pair) {
                    continue;
                }
                self.active_suspicions.insert(pair);

                let actually_crashed = self.nodes.get(&suspected_id).is_some_and(|n| !n.is_alive());

                let latency = if actually_crashed {
                    // Find the crash tick for this node.
                    self.metrics
                        .crashes
                        .iter()
                        .rev()
                        .find(|&&(_, n)| n == suspected_id)
                        .map(|&(crash_tick, _)| tick - crash_tick)
                } else {
                    None
                };

                self.metrics.record_detection(DetectionEvent {
                    tick,
                    node: suspected_id,
                    true_positive: actually_crashed,
                    latency,
                });
            }

            // Clear suspicions for nodes no longer in the suspected list.
            let current_suspected: HashSet<NodeId> =
                detector.suspected_nodes().into_iter().collect();
            self.active_suspicions
                .retain(|&(detector_node, suspected_id)| {
                    detector_node != node || current_suspected.contains(&suspected_id)
                });
        }

        // Re-schedule next detector tick.
        self.queue.schedule(Event {
            tick: tick + interval,
            kind: EventKind::DetectorTick { node },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_respects_max_tick() {
        let mut engine = Engine::new(100);
        engine.queue.schedule(Event {
            tick: 50,
            kind: EventKind::HeartbeatSend { from: 1 },
        });
        engine.queue.schedule(Event {
            tick: 200,
            kind: EventKind::HeartbeatSend { from: 2 },
        });
        engine.run();
        assert_eq!(engine.clock.now(), 50);
    }
}
