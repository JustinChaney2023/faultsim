use std::io::Write;
use std::path::Path;

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

    /// Fraction of detection events that were false positives.
    pub fn false_positive_rate(&self) -> f64 {
        if self.detections.is_empty() {
            return 0.0;
        }
        let fp = self.detections.iter().filter(|d| !d.true_positive).count();
        fp as f64 / self.detections.len() as f64
    }

    /// Mean detection latency across true-positive detections.
    pub fn mean_detection_latency(&self) -> Option<f64> {
        let latencies: Vec<f64> = self
            .detections
            .iter()
            .filter_map(|d| {
                if d.true_positive {
                    d.latency.map(|l| l as f64)
                } else {
                    None
                }
            })
            .collect();

        if latencies.is_empty() {
            None
        } else {
            Some(latencies.iter().sum::<f64>() / latencies.len() as f64)
        }
    }

    /// Average messages delivered per tick.
    pub fn messages_per_tick(&self, total_ticks: u64) -> f64 {
        if total_ticks == 0 {
            return 0.0;
        }
        self.message_count as f64 / total_ticks as f64
    }

    /// Export detection events to a CSV file.
    pub fn export_detections_csv(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "tick,node,true_positive,latency")?;
        for d in &self.detections {
            writeln!(
                f,
                "{},{},{},{}",
                d.tick,
                d.node,
                d.true_positive,
                d.latency.map_or(String::new(), |l| l.to_string())
            )?;
        }
        Ok(())
    }

    /// Export a summary row to a CSV file (appends if file exists).
    pub fn export_summary_csv(
        &self,
        path: &Path,
        total_ticks: u64,
        scenario_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let write_header = !path.exists();
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        if write_header {
            writeln!(
                f,
                "scenario,total_ticks,messages,messages_per_tick,detections,false_positive_rate,mean_detection_latency,crashes,recoveries"
            )?;
        }
        writeln!(
            f,
            "{},{},{},{:.4},{},{:.4},{},{},{}",
            scenario_name,
            total_ticks,
            self.message_count,
            self.messages_per_tick(total_ticks),
            self.detections.len(),
            self.false_positive_rate(),
            self.mean_detection_latency()
                .map_or("N/A".to_string(), |l| format!("{:.2}", l)),
            self.crashes.len(),
            self.recoveries.len(),
        )?;
        Ok(())
    }
}
