use crate::clock::{Clock, Tick};
use crate::event::{Event, EventKind, EventQueue};
use crate::metrics::MetricsCollector;

/// The simulation engine. Drives the event loop by pulling events from the
/// queue, advancing the clock, and dispatching to the appropriate handlers.
pub struct Engine {
    pub clock: Clock,
    pub queue: EventQueue,
    pub metrics: MetricsCollector,
    max_tick: Tick,
}

impl Engine {
    pub fn new(max_tick: Tick) -> Self {
        Self {
            clock: Clock::new(),
            queue: EventQueue::new(),
            metrics: MetricsCollector::new(),
            max_tick,
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
            EventKind::HeartbeatArrival { from, to } => {
                // TODO: Look up target node's detector, call on_heartbeat
                self.metrics.record_message(self.clock.now());
                let _ = (from, to);
            }
            EventKind::HeartbeatSend { from } => {
                // TODO: For each peer of `from`, compute delivery via network, schedule arrival
                // TODO: Re-schedule next HeartbeatSend for this node
                let _ = from;
            }
            EventKind::NodeCrash { node } => {
                // TODO: Set node state to Crashed, stop scheduling heartbeats
                self.metrics.record_crash(self.clock.now(), node);
            }
            EventKind::NodeRecover { node } => {
                // TODO: Set node state to Alive, resume heartbeats
                self.metrics.record_recovery(self.clock.now(), node);
            }
            EventKind::DetectorTick { node } => {
                // TODO: Call detector.on_tick, check suspected_nodes, record detections
                // TODO: Re-schedule next DetectorTick
                let _ = node;
            }
        }
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
