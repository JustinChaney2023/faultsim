use crate::clock::Tick;
use crate::node::NodeId;

/// Records events during simulation for post-run analysis.
#[derive(Debug, Default)]
pub struct MetricsCollector {
    /// Total messages delivered.
    pub message_count: u64,
    /// Detection events: (tick, node_id, was_true_positive).
    pub detections: Vec<DetectionEvent>,
    /// Crash events: (tick, node_id).
    pub crashes: Vec<(Tick, NodeId)>,
    /// Recovery events: (tick, node_id).
    pub recoveries: Vec<(Tick, NodeId)>,
}

/// A single detection event recorded by the metrics collector.
#[derive(Debug, Clone)]
pub struct DetectionEvent {
    pub tick: Tick,
    pub node: NodeId,
    pub true_positive: bool,
    /// Ticks between actual crash and detection (only meaningful for true positives).
    pub latency: Option<Tick>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_message(&mut self, _tick: Tick) {
        self.message_count += 1;
    }

    pub fn record_crash(&mut self, tick: Tick, node: NodeId) {
        self.crashes.push((tick, node));
    }

    pub fn record_recovery(&mut self, tick: Tick, node: NodeId) {
        self.recoveries.push((tick, node));
    }

    pub fn record_detection(&mut self, event: DetectionEvent) {
        self.detections.push(event);
    }

    // TODO: Implement summary statistics
    // - false_positive_rate() -> f64
    // - mean_detection_latency() -> f64
    // - messages_per_tick(total_ticks) -> f64
    // - recovery_times() -> Vec<Tick>

    // TODO: Implement CSV or JSON export for analysis scripts
}
