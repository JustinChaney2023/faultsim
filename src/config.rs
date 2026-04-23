use std::collections::HashMap;

use serde::Deserialize;

/// Top-level scenario configuration, deserialized from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct ScenarioConfig {
    pub simulation: SimulationConfig,
    pub cluster: ClusterConfig,
    pub network: NetworkConfigToml,
    pub detector: DetectorConfig,
    /// Fault injection schedule. Each entry describes a fault event.
    #[serde(default)]
    pub faults: Vec<FaultConfig>,
    /// Output/export options.
    pub output: Option<OutputConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimulationConfig {
    /// Maximum simulation ticks.
    pub max_ticks: u64,
    /// RNG seed for deterministic replay.
    pub seed: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClusterConfig {
    /// Number of nodes in the cluster.
    pub node_count: u32,
    /// Ticks between heartbeat sends.
    pub heartbeat_interval: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkConfigToml {
    /// Base one-way latency in ticks.
    pub base_latency: u64,
    /// Maximum jitter in ticks.
    pub jitter: u64,
    /// Message drop probability [0.0, 1.0].
    pub drop_probability: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetectorConfig {
    /// Which detection strategy to use.
    pub strategy: DetectorStrategy,
    /// Fixed-timeout value (used by FixedTimeout strategy).
    pub timeout: Option<u64>,
    /// EWMA alpha (used by Adaptive strategy).
    pub alpha: Option<f64>,
    /// Safety multiplier (used by Adaptive strategy).
    pub safety_multiplier: Option<f64>,
    /// Suspicion threshold (used by Gossip strategy).
    pub suspicion_threshold: Option<u32>,
    /// Ticks between gossip rounds (used by Gossip strategy).
    pub gossip_interval: Option<u64>,
    /// Number of random peers to gossip with each round (used by Gossip strategy).
    pub gossip_fanout: Option<u32>,
    /// φ suspicion threshold (used by PhiAccrual strategy). Common: 8, 12, 16.
    pub phi_threshold: Option<f64>,
    /// Sliding-window size for inter-arrival samples (used by PhiAccrual strategy).
    pub phi_window_size: Option<usize>,
    /// Minimum stddev floor in ticks (used by PhiAccrual strategy).
    pub phi_min_stddev: Option<f64>,
    /// Freeform parameters passed to the custom strategy.
    /// Define any f64 values you need:
    ///   [detector.params]
    ///   my_threshold = 5.0
    ///   my_window    = 100.0
    #[serde(default)]
    pub params: HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectorStrategy {
    FixedTimeout,
    Adaptive,
    Gossip,
    PhiAccrual,
    AdaptiveAccrual,
    /// User-defined strategy. Edit `src/detector/custom.rs` and set
    /// `strategy = "custom"` in your scenario TOML.
    Custom,
}

/// A single fault injection event in the schedule.
#[derive(Debug, Clone, Deserialize)]
pub struct FaultConfig {
    /// Tick at which the fault occurs.
    pub tick: u64,
    /// Type of fault.
    pub kind: FaultKind,
    /// Target node (for crash/recover).
    pub node: Option<u64>,
    /// Groups of node IDs for partition events. Each group can communicate
    /// internally but not with other groups.
    pub groups: Option<Vec<Vec<u64>>>,
}

/// Types of faults that can be injected.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    Crash,
    Recover,
    PartitionStart,
    PartitionEnd,
}

/// Output/export configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct OutputConfig {
    /// Directory to write results to. Defaults to "results/".
    pub dir: Option<String>,
    /// Export format: "csv" or "json".
    pub format: Option<String>,
    /// When true, record per-tick φ values from accrual detectors and export
    /// as `<name>_phi_log.csv` and `<name>_events.csv`.
    pub phi_log: Option<bool>,
}
