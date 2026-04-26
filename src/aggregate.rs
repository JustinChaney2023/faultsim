//! Aggregation of metrics across multiple simulation runs.
//!
//! Used by `sweep-seeds` (vary seed, fixed config) and `sweep` (vary one
//! parameter, fixed seed) to produce statistically meaningful summaries.

use std::io::Write;
use std::path::Path;

use crate::metrics::MetricsCollector;

// ── Per-run snapshot ─────────────────────────────────────────────────────────

/// Flat snapshot of all computed metrics for one simulation run.
#[derive(Debug, Clone)]
pub struct RunSnapshot {
    pub seed: u64,
    pub false_positive_rate: f64,
    /// Stored as f64 so it can participate in mean/stddev calculations.
    pub false_negative_count: f64,
    /// NaN when there are no true-positive detections in this run.
    pub mean_detection_latency: f64,
    pub p50_latency: f64,
    pub p95_latency: f64,
    pub p99_latency: f64,
    pub message_count: f64,
}

impl RunSnapshot {
    pub fn from_metrics(metrics: &MetricsCollector, seed: u64) -> Self {
        Self {
            seed,
            false_positive_rate: metrics.false_positive_rate(),
            false_negative_count: metrics.false_negative_count() as f64,
            mean_detection_latency: metrics.mean_detection_latency().unwrap_or(f64::NAN),
            p50_latency: metrics
                .detection_latency_percentile(50.0)
                .unwrap_or(f64::NAN),
            p95_latency: metrics
                .detection_latency_percentile(95.0)
                .unwrap_or(f64::NAN),
            p99_latency: metrics
                .detection_latency_percentile(99.0)
                .unwrap_or(f64::NAN),
            message_count: metrics.message_count as f64,
        }
    }
}

// ── Descriptive statistics ───────────────────────────────────────────────────

/// Mean, sample stddev, min, and max for one metric across N runs.
/// NaN values in the input are excluded (metric not applicable to that run).
#[derive(Debug, Clone)]
pub struct Stat {
    pub mean: f64,
    pub stddev: f64,
    pub min: f64,
    pub max: f64,
    /// Number of non-NaN runs that contributed.
    pub n: usize,
}

impl Stat {
    pub fn from_values(values: &[f64]) -> Self {
        let valid: Vec<f64> = values.iter().copied().filter(|v| !v.is_nan()).collect();
        let n = valid.len();
        if n == 0 {
            return Self {
                mean: f64::NAN,
                stddev: f64::NAN,
                min: f64::NAN,
                max: f64::NAN,
                n: 0,
            };
        }
        let mean = valid.iter().sum::<f64>() / n as f64;
        // Bessel-corrected sample variance (N-1); falls back to 0 for n=1.
        let variance = if n < 2 {
            0.0
        } else {
            valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        };
        Self {
            mean,
            stddev: variance.sqrt(),
            min: valid.iter().cloned().fold(f64::INFINITY, f64::min),
            max: valid.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            n,
        }
    }

    /// Format as "mean ± stddev  [min, max]", or "N/A" when n == 0.
    pub fn display(&self) -> String {
        if self.n == 0 {
            "N/A".to_string()
        } else {
            format!(
                "{:.4} ± {:.4}  [{:.2}, {:.2}]",
                self.mean, self.stddev, self.min, self.max
            )
        }
    }
}

// ── Aggregated metrics ────────────────────────────────────────────────────────

macro_rules! stat_field {
    ($runs:expr, $field:ident) => {
        Stat::from_values(&$runs.iter().map(|r| r.$field).collect::<Vec<_>>())
    };
}

/// All metrics aggregated over a set of runs.
#[derive(Debug)]
pub struct AggregatedMetrics {
    pub total_runs: usize,
    pub false_positive_rate: Stat,
    pub false_negative_count: Stat,
    pub mean_detection_latency: Stat,
    pub p50_latency: Stat,
    pub p95_latency: Stat,
    pub p99_latency: Stat,
    pub message_count: Stat,
}

impl AggregatedMetrics {
    pub fn from_snapshots(runs: &[RunSnapshot]) -> Self {
        Self {
            total_runs: runs.len(),
            false_positive_rate: stat_field!(runs, false_positive_rate),
            false_negative_count: stat_field!(runs, false_negative_count),
            mean_detection_latency: stat_field!(runs, mean_detection_latency),
            p50_latency: stat_field!(runs, p50_latency),
            p95_latency: stat_field!(runs, p95_latency),
            p99_latency: stat_field!(runs, p99_latency),
            message_count: stat_field!(runs, message_count),
        }
    }

    /// Print a human-readable table to stdout.
    pub fn print(&self) {
        println!("=== Aggregated Results ({} runs) ===", self.total_runs);
        println!("{:<28}  mean ± stddev  [min, max]", "Metric");
        println!("{}", "-".repeat(70));
        println!(
            "{:<28}  {}",
            "False positive rate",
            self.false_positive_rate.display()
        );
        println!(
            "{:<28}  {}",
            "False negatives",
            self.false_negative_count.display()
        );
        println!(
            "{:<28}  {}",
            "Mean detection latency",
            self.mean_detection_latency.display()
        );
        println!(
            "{:<28}  {}",
            "p50 detection latency",
            self.p50_latency.display()
        );
        println!(
            "{:<28}  {}",
            "p95 detection latency",
            self.p95_latency.display()
        );
        println!(
            "{:<28}  {}",
            "p99 detection latency",
            self.p99_latency.display()
        );
        println!(
            "{:<28}  {}",
            "Messages delivered",
            self.message_count.display()
        );
    }

    /// Write aggregated stats to a CSV file (metric per row).
    pub fn export_csv(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "metric,n,mean,stddev,min,max")?;
        let row = |name: &str, s: &Stat| -> String {
            if s.n == 0 {
                format!("{},{},N/A,N/A,N/A,N/A", name, s.n)
            } else {
                format!(
                    "{},{},{:.6},{:.6},{:.2},{:.2}",
                    name, s.n, s.mean, s.stddev, s.min, s.max
                )
            }
        };
        writeln!(
            f,
            "{}",
            row("false_positive_rate", &self.false_positive_rate)
        )?;
        writeln!(
            f,
            "{}",
            row("false_negative_count", &self.false_negative_count)
        )?;
        writeln!(
            f,
            "{}",
            row("mean_detection_latency", &self.mean_detection_latency)
        )?;
        writeln!(f, "{}", row("p50_latency", &self.p50_latency))?;
        writeln!(f, "{}", row("p95_latency", &self.p95_latency))?;
        writeln!(f, "{}", row("p99_latency", &self.p99_latency))?;
        writeln!(f, "{}", row("message_count", &self.message_count))?;
        Ok(())
    }
}

// ── CSV exports ───────────────────────────────────────────────────────────────

/// Write one row per run to a CSV (for sweep-seeds output).
pub fn export_runs_csv(
    runs: &[RunSnapshot],
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut f = std::fs::File::create(path)?;
    writeln!(
        f,
        "seed,false_positive_rate,false_negatives,mean_detection_latency,\
         p50_latency,p95_latency,p99_latency,message_count"
    )?;
    let fmt = |v: f64| {
        if v.is_nan() {
            "N/A".to_string()
        } else {
            format!("{:.4}", v)
        }
    };
    for r in runs {
        writeln!(
            f,
            "{},{:.4},{},{},{},{},{},{}",
            r.seed,
            r.false_positive_rate,
            r.false_negative_count,
            fmt(r.mean_detection_latency),
            fmt(r.p50_latency),
            fmt(r.p95_latency),
            fmt(r.p99_latency),
            r.message_count,
        )?;
    }
    Ok(())
}

/// Write one row per parameter value to a CSV (for sweep output).
/// `label` is the parameter name used as the first column header.
pub fn export_sweep_csv(
    rows: &[(f64, RunSnapshot)],
    param_label: &str,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut f = std::fs::File::create(path)?;
    writeln!(
        f,
        "{},false_positive_rate,false_negatives,mean_detection_latency,\
         p50_latency,p95_latency,p99_latency,message_count",
        param_label
    )?;
    let fmt = |v: f64| {
        if v.is_nan() {
            "N/A".to_string()
        } else {
            format!("{:.4}", v)
        }
    };
    for (param_val, r) in rows {
        writeln!(
            f,
            "{:.6},{:.4},{},{},{},{},{},{}",
            param_val,
            r.false_positive_rate,
            r.false_negative_count,
            fmt(r.mean_detection_latency),
            fmt(r.p50_latency),
            fmt(r.p95_latency),
            fmt(r.p99_latency),
            r.message_count,
        )?;
    }
    Ok(())
}
