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
    /// Per-tick φ values from accrual detectors (only populated when phi logging is enabled).
    pub phi_log: Vec<PhiLogEntry>,
}

/// One φ sample: observer node watching observed node at a given tick.
#[derive(Debug, Clone)]
pub struct PhiLogEntry {
    pub tick: Tick,
    pub observer: NodeId,
    pub observed: NodeId,
    pub phi: f64,
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

    /// Increment the delivered-message counter.
    /// The `_tick` parameter is reserved for future per-interval message-rate
    /// tracking and is intentionally unused for now.
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
        let latencies = self.true_positive_latencies();
        if latencies.is_empty() {
            None
        } else {
            Some(latencies.iter().sum::<f64>() / latencies.len() as f64)
        }
    }

    /// Detection latency at the given percentile (0–100) across true-positive detections.
    /// Returns None if there are no true-positive detections.
    pub fn detection_latency_percentile(&self, p: f64) -> Option<f64> {
        let mut latencies = self.true_positive_latencies();
        if latencies.is_empty() {
            return None;
        }
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Nearest-rank method.
        let idx = ((p / 100.0) * latencies.len() as f64).ceil() as usize;
        let idx = idx.saturating_sub(1).min(latencies.len() - 1);
        Some(latencies[idx])
    }

    /// Number of crash events that were never followed by a true-positive detection.
    /// A crash is a false negative if no TP detection for that node exists at or after
    /// the crash tick.
    pub fn false_negative_count(&self) -> usize {
        self.crashes
            .iter()
            .filter(|&&(crash_tick, node_id)| {
                !self
                    .detections
                    .iter()
                    .any(|d| d.node == node_id && d.true_positive && d.tick >= crash_tick)
            })
            .count()
    }

    /// Sorted true-positive latencies as f64, used by mean and percentile methods.
    fn true_positive_latencies(&self) -> Vec<f64> {
        self.detections
            .iter()
            .filter_map(|d| {
                if d.true_positive {
                    d.latency.map(|l| l as f64)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Average messages delivered per tick.
    pub fn messages_per_tick(&self, total_ticks: u64) -> f64 {
        if total_ticks == 0 {
            return 0.0;
        }
        self.message_count as f64 / total_ticks as f64
    }

    /// Append one φ sample to the phi log.
    pub fn record_phi(&mut self, entry: PhiLogEntry) {
        self.phi_log.push(entry);
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
        wall_time_ms: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let write_header = !path.exists();
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        if write_header {
            writeln!(
                f,
                "scenario,total_ticks,messages,messages_per_tick,detections,false_positive_rate,false_negatives,mean_detection_latency,p50_latency,p95_latency,p99_latency,crashes,recoveries,wall_time_ms"
            )?;
        }
        let fmt_lat = |v: Option<f64>| v.map_or("N/A".to_string(), |l| format!("{:.2}", l));
        writeln!(
            f,
            "{},{},{},{:.4},{},{:.4},{},{},{},{},{},{},{},{:.3}",
            scenario_name,
            total_ticks,
            self.message_count,
            self.messages_per_tick(total_ticks),
            self.detections.len(),
            self.false_positive_rate(),
            self.false_negative_count(),
            fmt_lat(self.mean_detection_latency()),
            fmt_lat(self.detection_latency_percentile(50.0)),
            fmt_lat(self.detection_latency_percentile(95.0)),
            fmt_lat(self.detection_latency_percentile(99.0)),
            self.crashes.len(),
            self.recoveries.len(),
            wall_time_ms, // written as float with 3 decimal places
        )?;
        Ok(())
    }

    /// Export per-tick φ values to a CSV file.
    /// Columns: tick, observer, observed, phi
    pub fn export_phi_log_csv(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "tick,observer,observed,phi")?;
        for e in &self.phi_log {
            // Cap at 30 for readability; infinity is written as inf.
            let phi_str = if e.phi.is_infinite() {
                "inf".to_string()
            } else {
                format!("{:.4}", e.phi.min(30.0))
            };
            writeln!(f, "{},{},{},{}", e.tick, e.observer, e.observed, phi_str)?;
        }
        Ok(())
    }

    /// Export detection events to a JSON array file.
    pub fn export_detections_json(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "[")?;
        let last = self.detections.len().saturating_sub(1);
        for (i, d) in self.detections.iter().enumerate() {
            let comma = if i < last { "," } else { "" };
            let latency = d.latency.map_or("null".to_string(), |l| l.to_string());
            writeln!(
                f,
                "  {{\"tick\":{},\"node\":{},\"true_positive\":{},\"latency\":{}}}{}",
                d.tick, d.node, d.true_positive, latency, comma
            )?;
        }
        writeln!(f, "]")?;
        Ok(())
    }

    /// Export a summary object to a JSON file.
    pub fn export_summary_json(
        &self,
        path: &Path,
        total_ticks: u64,
        scenario_name: &str,
        wall_time_ms: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let null_or = |v: Option<f64>| v.map_or("null".to_string(), |l| format!("{:.4}", l));
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "{{")?;
        writeln!(f, "  \"scenario\": \"{}\",", scenario_name)?;
        writeln!(f, "  \"total_ticks\": {},", total_ticks)?;
        writeln!(f, "  \"messages\": {},", self.message_count)?;
        writeln!(
            f,
            "  \"messages_per_tick\": {:.4},",
            self.messages_per_tick(total_ticks)
        )?;
        writeln!(f, "  \"detections\": {},", self.detections.len())?;
        writeln!(
            f,
            "  \"false_positive_rate\": {:.4},",
            self.false_positive_rate()
        )?;
        writeln!(f, "  \"false_negatives\": {},", self.false_negative_count())?;
        writeln!(
            f,
            "  \"mean_detection_latency\": {},",
            null_or(self.mean_detection_latency())
        )?;
        writeln!(
            f,
            "  \"p50_latency\": {},",
            null_or(self.detection_latency_percentile(50.0))
        )?;
        writeln!(
            f,
            "  \"p95_latency\": {},",
            null_or(self.detection_latency_percentile(95.0))
        )?;
        writeln!(
            f,
            "  \"p99_latency\": {},",
            null_or(self.detection_latency_percentile(99.0))
        )?;
        writeln!(f, "  \"crashes\": {},", self.crashes.len())?;
        writeln!(f, "  \"recoveries\": {},", self.recoveries.len())?;
        writeln!(f, "  \"wall_time_ms\": {:.3}", wall_time_ms)?;
        writeln!(f, "}}")?;
        Ok(())
    }

    /// Export crash and recovery events to a CSV file.
    /// Columns: tick, kind, node  — used by the φ timeline plot script.
    pub fn export_events_csv(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "tick,kind,node")?;
        for &(tick, node) in &self.crashes {
            writeln!(f, "{},crash,{}", tick, node)?;
        }
        for &(tick, node) in &self.recoveries {
            writeln!(f, "{},recovery,{}", tick, node)?;
        }
        Ok(())
    }
}
