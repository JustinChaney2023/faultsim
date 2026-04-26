use std::collections::HashMap;
use std::path::Path;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::config::{DetectorStrategy, FaultKind, ScenarioConfig};
use crate::detector::adaptive::AdaptiveDetector;
use crate::detector::adaptive_accrual::AdaptiveAccrualDetector;
use crate::detector::custom::CustomDetector;
use crate::detector::fixed_timeout::FixedTimeoutDetector;
use crate::detector::gossip::GossipDetector;
use crate::detector::phi_accrual::PhiAccrualDetector;
use crate::detector::FailureDetector;
use crate::engine::Engine;
use crate::event::{Event, EventKind};
use crate::metrics::MetricsCollector;
use crate::network::{Network, NetworkConfig};
use crate::node::Node;

/// Loads a scenario configuration from a TOML file.
pub fn load_config(path: &Path) -> Result<ScenarioConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: ScenarioConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Builds a fully wired simulation engine from the given configuration.
pub fn build_engine(config: &ScenarioConfig, seed_override: Option<u64>) -> Engine {
    let seed = seed_override.unwrap_or(config.simulation.seed);
    let mut rng = StdRng::seed_from_u64(seed);

    let network = Network::new(NetworkConfig {
        base_latency: config.network.base_latency,
        jitter: config.network.jitter,
        drop_probability: config.network.drop_probability,
    });

    let node_ids: Vec<u64> = (1..=config.cluster.node_count as u64).collect();
    let heartbeat_interval = config.cluster.heartbeat_interval;
    // Detectors poll at the same rate as heartbeats. This means detection
    // latency is bounded below by one heartbeat interval, which is the
    // standard model in the literature.
    let detector_interval = heartbeat_interval;

    // Create nodes, each with a peers list of all other nodes.
    let mut nodes = HashMap::new();
    for &id in &node_ids {
        let peers: Vec<u64> = node_ids.iter().copied().filter(|&nid| nid != id).collect();
        nodes.insert(
            id,
            Node::with_config(id, heartbeat_interval, detector_interval, peers),
        );
    }

    // Create a detector for each node.
    let mut detectors: HashMap<u64, Box<dyn FailureDetector>> = HashMap::new();
    for &id in &node_ids {
        let monitored: Vec<u64> = node_ids.iter().copied().filter(|&nid| nid != id).collect();
        let detector: Box<dyn FailureDetector> = match config.detector.strategy {
            DetectorStrategy::FixedTimeout => {
                let timeout = config.detector.timeout.unwrap_or(200);
                Box::new(FixedTimeoutDetector::new(timeout, monitored))
            }
            DetectorStrategy::Adaptive => {
                let alpha = config.detector.alpha.unwrap_or(0.5);
                let safety = config.detector.safety_multiplier.unwrap_or(2.0);
                Box::new(AdaptiveDetector::new(alpha, safety, monitored))
            }
            DetectorStrategy::Gossip => {
                let threshold = config.detector.suspicion_threshold.unwrap_or(3);
                let local_timeout = config.detector.timeout.unwrap_or(200);
                let gossip_interval = config.detector.gossip_interval.unwrap_or(50);
                let gossip_fanout = config.detector.gossip_fanout.unwrap_or(3);
                Box::new(GossipDetector::new(
                    threshold,
                    local_timeout,
                    monitored,
                    id,
                    gossip_interval,
                    gossip_fanout,
                ))
            }
            DetectorStrategy::PhiAccrual => {
                let threshold = config.detector.phi_threshold.unwrap_or(8.0);
                let window = config.detector.phi_window_size.unwrap_or(100);
                let min_stddev = config.detector.phi_min_stddev.unwrap_or(1.0);
                Box::new(PhiAccrualDetector::new(
                    threshold, window, min_stddev, monitored,
                ))
            }
            DetectorStrategy::AdaptiveAccrual => {
                // Reuses phi_threshold and phi_window_size (same semantics) but
                // recommended thresholds are smaller — empirical CDF is bounded
                // below by 1/(n+1), so a threshold of 2 is roughly equivalent
                // to "no historical sample was this delayed".
                let threshold = config.detector.phi_threshold.unwrap_or(2.0);
                let window = config.detector.phi_window_size.unwrap_or(100);
                Box::new(AdaptiveAccrualDetector::new(threshold, window, monitored))
            }
            DetectorStrategy::Custom => {
                Box::new(CustomDetector::new(&config.detector.params, monitored))
            }
        };
        detectors.insert(id, detector);
    }

    let mut engine = Engine::with_simulation(
        config.simulation.max_ticks,
        nodes,
        network,
        detectors,
        rng.clone(),
    );

    // Schedule staggered initial events for each node.
    // Heartbeats start immediately (staggered within the first interval).
    // Detector ticks start after one full interval so heartbeats can arrive first.
    for &id in &node_ids {
        let hb_offset = rng.gen_range(1..=heartbeat_interval);
        engine.queue.schedule(Event {
            tick: hb_offset,
            kind: EventKind::HeartbeatSend { from: id },
        });

        let dt_offset = heartbeat_interval + rng.gen_range(1..=detector_interval);
        engine.queue.schedule(Event {
            tick: dt_offset,
            kind: EventKind::DetectorTick { node: id },
        });
    }

    // Schedule initial gossip rounds for gossip strategy.
    if config.detector.strategy == DetectorStrategy::Gossip {
        let gossip_interval = config.detector.gossip_interval.unwrap_or(50);
        for &id in &node_ids {
            let gossip_offset = heartbeat_interval + rng.gen_range(1..=gossip_interval);
            engine.queue.schedule(Event {
                tick: gossip_offset,
                kind: EventKind::GossipRound { from: id },
            });
        }
    }

    // Schedule fault injection events from config.
    for fault in &config.faults {
        let kind = match fault.kind {
            FaultKind::Crash => {
                let node = fault.node.expect("crash fault requires 'node'");
                EventKind::NodeCrash { node }
            }
            FaultKind::Recover => {
                let node = fault.node.expect("recover fault requires 'node'");
                EventKind::NodeRecover { node }
            }
            FaultKind::PartitionStart => {
                let groups = fault
                    .groups
                    .clone()
                    .expect("partition_start fault requires 'groups'");
                EventKind::PartitionStart { groups }
            }
            FaultKind::PartitionEnd => EventKind::PartitionEnd,
        };
        engine.queue.schedule(Event {
            tick: fault.tick,
            kind,
        });
    }

    // Enable φ timeline logging if requested in the output config.
    if config
        .output
        .as_ref()
        .and_then(|o| o.phi_log)
        .unwrap_or(false)
    {
        engine.phi_log_enabled = true;
    }

    engine
}

/// Prints a human-readable summary of simulation results.
pub fn print_summary(metrics: &MetricsCollector, max_ticks: u64) {
    let fmt_lat = |v: Option<f64>| v.map_or("N/A".to_string(), |l| format!("{:.2} ticks", l));
    println!("=== Simulation Summary ===");
    println!("Total ticks:            {}", max_ticks);
    println!("Messages delivered:     {}", metrics.message_count);
    println!(
        "Messages per tick:      {:.2}",
        metrics.messages_per_tick(max_ticks)
    );
    println!("Crashes:                {}", metrics.crashes.len());
    println!("Recoveries:             {}", metrics.recoveries.len());
    println!("Detection events:       {}", metrics.detections.len());
    println!(
        "False positive rate:    {:.4}",
        metrics.false_positive_rate()
    );
    println!("False negatives:        {}", metrics.false_negative_count());
    println!(
        "Mean detection latency: {}",
        fmt_lat(metrics.mean_detection_latency())
    );
    println!(
        "p50 detection latency:  {}",
        fmt_lat(metrics.detection_latency_percentile(50.0))
    );
    println!(
        "p95 detection latency:  {}",
        fmt_lat(metrics.detection_latency_percentile(95.0))
    );
    println!(
        "p99 detection latency:  {}",
        fmt_lat(metrics.detection_latency_percentile(99.0))
    );
}
