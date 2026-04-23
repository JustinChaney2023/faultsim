use std::any::Any;
use std::collections::{HashMap, VecDeque};

use crate::clock::Tick;
use crate::detector::FailureDetector;
use crate::node::NodeId;

/// The φ Accrual Failure Detector (Hayashibara, Defago, Yared, Katayama, 2004).
///
/// Maintains a sliding window of heartbeat inter-arrival times per monitored
/// node. When queried, computes a continuous suspicion level φ from the
/// probability that a heartbeat *should* have been received by now under a
/// normal-distribution model of inter-arrival times. A node is suspected when
/// φ exceeds `threshold`.
///
/// This is the canonical SOTA accrual detector — used as the reference baseline
/// in Cassandra and Akka. Distinct from our `AdaptiveDetector`, which only
/// applies a multiplier to an EWMA of the mean (no distributional model and no
/// continuous suspicion level).
///
/// Reference: Hayashibara et al., "The φ Accrual Failure Detector", SRDS 2004.
#[derive(Debug)]
pub struct PhiAccrualDetector {
    /// Suspicion threshold. Common values: 8 (aggressive), 12, 16 (conservative).
    threshold: f64,
    /// Maximum number of inter-arrival samples kept per node.
    window_size: usize,
    /// Minimum stddev floor — prevents φ from blowing up when samples are
    /// near-identical (e.g., a clean network with no jitter). Expressed in ticks.
    min_stddev: f64,
    /// Per-node sliding window of inter-arrival times (in ticks).
    intervals: HashMap<NodeId, VecDeque<u64>>,
    /// Per-node tick of last heartbeat received.
    last_heartbeat: HashMap<NodeId, Tick>,
    /// Nodes being monitored.
    monitored: Vec<NodeId>,
    /// Current simulation tick, updated via on_tick.
    current_tick: Tick,
}

impl PhiAccrualDetector {
    pub fn new(
        threshold: f64,
        window_size: usize,
        min_stddev: f64,
        monitored: Vec<NodeId>,
    ) -> Self {
        Self {
            threshold,
            window_size,
            min_stddev,
            intervals: HashMap::new(),
            last_heartbeat: HashMap::new(),
            monitored,
            current_tick: 0,
        }
    }

    /// Returns the current φ value for a node, or `None` if there are not yet
    /// enough samples to estimate the inter-arrival distribution. Exposed for
    /// inspection and testing — the engine itself only uses `suspected_nodes`.
    pub fn phi(&self, node: NodeId) -> Option<f64> {
        let last = *self.last_heartbeat.get(&node)?;
        let samples = self.intervals.get(&node)?;
        if samples.len() < 2 {
            return None;
        }
        let (mean, stddev) = mean_stddev(samples.iter().copied());
        let stddev = stddev.max(self.min_stddev);
        let delta = self.current_tick.saturating_sub(last) as f64;
        Some(phi_value(delta, mean, stddev))
    }
}

impl FailureDetector for PhiAccrualDetector {
    fn on_heartbeat(&mut self, from: NodeId, tick: Tick) {
        if let Some(&prev) = self.last_heartbeat.get(&from) {
            let interval = tick - prev;
            let window = self.intervals.entry(from).or_default();
            window.push_back(interval);
            while window.len() > self.window_size {
                window.pop_front();
            }
        }
        self.last_heartbeat.insert(from, tick);
    }

    fn on_tick(&mut self, tick: Tick) {
        self.current_tick = tick;
    }

    fn suspected_nodes(&self) -> Vec<NodeId> {
        if self.current_tick == 0 {
            return Vec::new();
        }
        self.monitored
            .iter()
            .copied()
            .filter(|&node| match self.phi(node) {
                Some(phi) => phi > self.threshold,
                // No samples yet: don't suspect. The detector is still warming up.
                None => false,
            })
            .collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn phi_for_node(&self, node: NodeId) -> Option<f64> {
        self.phi(node)
    }
}

fn mean_stddev<I: Iterator<Item = u64> + Clone>(samples: I) -> (f64, f64) {
    let values: Vec<f64> = samples.map(|v| v as f64).collect();
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    (mean, variance.sqrt())
}

/// φ(t) = -log10(P_later(t)) where P_later is the probability that the next
/// heartbeat arrives later than `delta` under a Normal(mean, stddev) model.
fn phi_value(delta: f64, mean: f64, stddev: f64) -> f64 {
    let p_later = 1.0 - normal_cdf(delta, mean, stddev);
    if p_later <= 0.0 {
        // Numerically saturated — the heartbeat is so overdue that under the
        // model the probability has rounded to 0. Treat as definitely failed.
        f64::INFINITY
    } else {
        -p_later.log10()
    }
}

fn normal_cdf(x: f64, mean: f64, stddev: f64) -> f64 {
    0.5 * (1.0 + erf((x - mean) / (stddev * std::f64::consts::SQRT_2)))
}

/// Abramowitz & Stegun 7.1.26 approximation of erf. Max error ~1.5e-7.
fn erf(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_samples_means_not_suspected() {
        let mut det = PhiAccrualDetector::new(8.0, 100, 1.0, vec![1, 2]);
        det.on_tick(500);
        assert!(det.suspected_nodes().is_empty());
    }

    #[test]
    fn regular_heartbeats_keep_phi_low() {
        let mut det = PhiAccrualDetector::new(8.0, 100, 1.0, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        // Query right after the most recent heartbeat — node should look healthy.
        det.on_tick(2010);
        assert!(det.suspected_nodes().is_empty());
        let phi = det.phi(1).expect("phi should be computable");
        assert!(phi < 1.0, "expected low phi for fresh heartbeat, got {phi}");
    }

    #[test]
    fn long_silence_drives_phi_above_threshold() {
        let mut det = PhiAccrualDetector::new(8.0, 100, 1.0, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        // Many intervals after the last heartbeat with no further messages.
        det.on_tick(5000);
        let phi = det.phi(1).expect("phi should be computable");
        assert!(phi > 8.0, "expected high phi after long silence, got {phi}");
        assert_eq!(det.suspected_nodes(), vec![1]);
    }

    #[test]
    fn phi_increases_monotonically_with_delay() {
        let mut det = PhiAccrualDetector::new(8.0, 100, 1.0, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        det.on_tick(2100);
        let phi_low = det.phi(1).unwrap();
        det.on_tick(2500);
        let phi_high = det.phi(1).unwrap();
        assert!(
            phi_high > phi_low,
            "phi should grow as delay grows: {phi_low} -> {phi_high}"
        );
    }

    #[test]
    fn window_size_is_respected() {
        let mut det = PhiAccrualDetector::new(8.0, 5, 1.0, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        let window_len = det.intervals.get(&1).map(|w| w.len()).unwrap_or(0);
        assert_eq!(window_len, 5);
    }

    #[test]
    fn erf_matches_known_values() {
        // Spot-check against tabulated values.
        assert!((erf(0.0) - 0.0).abs() < 1e-6);
        assert!((erf(1.0) - 0.8427007).abs() < 1e-5);
        assert!((erf(-1.0) + 0.8427007).abs() < 1e-5);
        assert!((erf(2.0) - 0.9953223).abs() < 1e-5);
    }
}
