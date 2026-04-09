use serde::Deserialize;

/// Top-level scenario configuration, deserialized from TOML.
#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct SimulationConfig {
    /// Maximum simulation ticks.
    pub max_ticks: u64,
    /// RNG seed for deterministic replay.
    pub seed: u64,
}

#[derive(Debug, Deserialize)]
pub struct ClusterConfig {
    /// Number of nodes in the cluster.
    pub node_count: u32,
    /// Ticks between heartbeat sends.
    pub heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfigToml {
    /// Base one-way latency in ticks.
    pub base_latency: u64,
    /// Maximum jitter in ticks.
    pub jitter: u64,
    /// Message drop probability [0.0, 1.0].
    pub drop_probability: f64,
}

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectorStrategy {
    FixedTimeout,
    Adaptive,
    Gossip,
}

/// A single fault injection event in the schedule.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    Crash,
    Recover,
    PartitionStart,
    PartitionEnd,
}

/// Output/export configuration.
#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    /// Directory to write results to. Defaults to "results/".
    pub dir: Option<String>,
    /// Export format: "csv" or "json".
    pub format: Option<String>,
}
