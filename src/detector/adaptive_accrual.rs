use std::any::Any;
use std::collections::{HashMap, VecDeque};

use crate::clock::Tick;
use crate::detector::FailureDetector;
use crate::node::NodeId;

/// Adaptive accrual failure detector — Satzger, Pietzowski, Trumler, Ungerer,
/// "A new adaptive accrual failure detector for dependable distributed
/// systems", SAC 2007.
///
/// Like φ-accrual it produces a continuous suspicion level from a sliding
/// window of inter-arrival times. The key difference: φ-accrual fits a
/// Normal(μ, σ) to the window, whereas this detector uses the **empirical
/// CDF** — P_later(Δ) is just the fraction of historical inter-arrivals that
/// were ≥ Δ. This drops the parametric assumption, which makes the detector
/// robust under non-normal jitter distributions (heavy-tailed, bimodal, etc.).
///
/// Suspicion: φ = −log₁₀(P_later). A node is suspected when φ > threshold.
///
/// Reference: Satzger et al., SAC 2007. The same idea was later extended by
/// NFD-A (Xiong et al., 2012); we implement the cleaner 2007 version as our
/// "newer SOTA" baseline.
#[derive(Debug)]
pub struct AdaptiveAccrualDetector {
    /// Suspicion threshold. φ > threshold ⇒ node suspected.
    threshold: f64,
    /// Maximum number of inter-arrival samples kept per node.
    window_size: usize,
    /// Per-node sliding window of inter-arrival times (in ticks).
    intervals: HashMap<NodeId, VecDeque<u64>>,
    /// Per-node tick of last heartbeat received.
    last_heartbeat: HashMap<NodeId, Tick>,
    /// Nodes being monitored.
    monitored: Vec<NodeId>,
    /// Current simulation tick, updated via on_tick.
    current_tick: Tick,
}

impl AdaptiveAccrualDetector {
    pub fn new(threshold: f64, window_size: usize, monitored: Vec<NodeId>) -> Self {
        Self {
            threshold,
            window_size,
            intervals: HashMap::new(),
            last_heartbeat: HashMap::new(),
            monitored,
            current_tick: 0,
        }
    }

    /// Returns the current φ value for a node, or `None` if there is not
    /// enough history to estimate the empirical distribution.
    pub fn phi(&self, node: NodeId) -> Option<f64> {
        let last = *self.last_heartbeat.get(&node)?;
        let samples = self.intervals.get(&node)?;
        if samples.len() < 2 {
            return None;
        }
        let delta = self.current_tick.saturating_sub(last);
        let n = samples.len() as f64;
        let count_ge = samples.iter().filter(|&&s| s >= delta).count() as f64;
        let p_later = count_ge / n;
        if p_later <= 0.0 {
            // Δ exceeds every observed inter-arrival — saturate. Satzger
            // explicitly notes this as the inherent boundary of the empirical
            // approach: without a parametric tail, the detector cannot
            // distinguish "very late" from "infinitely late."
            Some(f64::INFINITY)
        } else {
            Some(-p_later.log10())
        }
    }
}

impl FailureDetector for AdaptiveAccrualDetector {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_samples_means_not_suspected() {
        let mut det = AdaptiveAccrualDetector::new(2.0, 100, vec![1, 2]);
        det.on_tick(500);
        assert!(det.suspected_nodes().is_empty());
    }

    #[test]
    fn regular_heartbeats_keep_phi_low() {
        let mut det = AdaptiveAccrualDetector::new(2.0, 100, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        det.on_tick(2010);
        assert!(det.suspected_nodes().is_empty());
        let phi = det.phi(1).expect("phi should be computable");
        assert!(phi < 1.0, "expected low phi for fresh heartbeat, got {phi}");
    }

    #[test]
    fn long_silence_drives_phi_above_threshold() {
        let mut det = AdaptiveAccrualDetector::new(2.0, 100, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        det.on_tick(5000);
        let phi = det.phi(1).expect("phi should be computable");
        assert!(phi > 2.0, "expected high phi after long silence, got {phi}");
        assert_eq!(det.suspected_nodes(), vec![1]);
    }

    #[test]
    fn empirical_cdf_handles_bimodal_distribution() {
        // Bimodal samples: half at 50 ticks, half at 150 ticks. A Normal fit
        // would give μ=100, σ=50, badly misrepresenting the actual distribution.
        // Empirical CDF should accept Δ=120 as plausible (within range of larger mode).
        let mut det = AdaptiveAccrualDetector::new(2.0, 100, vec![1]);
        let mut tick = 0u64;
        for _ in 0..50 {
            tick += 50;
            det.on_heartbeat(1, tick);
            tick += 150;
            det.on_heartbeat(1, tick);
        }
        det.on_tick(tick + 120);
        let phi = det.phi(1).expect("phi should be computable");
        // Roughly half of samples are ≥ 120 → P_later ≈ 0.5 → φ ≈ 0.30.
        // Definitely should not be over the threshold of 2.0.
        assert!(
            phi < 1.0,
            "bimodal: expected modest phi at Δ=120, got {phi}"
        );
    }

    #[test]
    fn phi_increases_monotonically_with_delay() {
        let mut det = AdaptiveAccrualDetector::new(2.0, 100, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        det.on_tick(2100);
        let phi_low = det.phi(1).unwrap();
        det.on_tick(2500);
        let phi_high = det.phi(1).unwrap();
        assert!(
            phi_high > phi_low,
            "phi should grow with delay: {phi_low} -> {phi_high}"
        );
    }

    #[test]
    fn window_size_is_respected() {
        let mut det = AdaptiveAccrualDetector::new(2.0, 5, vec![1]);
        for t in (100..=2000).step_by(100) {
            det.on_heartbeat(1, t);
        }
        let len = det.intervals.get(&1).map(|w| w.len()).unwrap_or(0);
        assert_eq!(len, 5);
    }
}
