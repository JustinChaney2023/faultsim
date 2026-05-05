#!/usr/bin/env python3
"""
Plot faultsim experiment results from a summary.csv file.

Usage:
    python scripts/plot_results.py results/all/summary.csv
    python scripts/plot_results.py results/all/summary.csv --output results/figures
    python scripts/plot_results.py results/sweep/crash_phi_accrual_phi_threshold.csv --sweep phi_threshold

Requires: pip install matplotlib pandas
"""

import argparse
import os
import sys

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import pandas as pd
except ImportError:
    print("Error: matplotlib and pandas are required.")
    print("  pip install matplotlib pandas")
    sys.exit(1)


# ── Publication style ──────────────────────────────────────────────────────────

plt.rcParams.update({
    "font.family":       "sans-serif",
    "font.size":         10,
    "axes.titlesize":    11,
    "axes.labelsize":    10,
    "xtick.labelsize":   9,
    "ytick.labelsize":   9,
    "legend.fontsize":   9,
    "figure.dpi":        300,
    "savefig.dpi":       300,
    "axes.spines.top":   False,
    "axes.spines.right": False,
})


# ── Detector metadata ──────────────────────────────────────────────────────────

# Canonical crash scenario → display label
CRASH_SCENARIO_LABEL = {
    "crash_recovery":         "Fixed-Timeout",
    "crash_adaptive":         "EWMA-Adaptive",
    "crash_gossip":           "Gossip",
    "crash_phi_accrual":      "φ-Accrual",
    "crash_adaptive_accrual": "Adaptive-Accrual",
}

# High-jitter scenario → same display labels (for panel 3 of comparison figure)
JITTER_SCENARIO_LABEL = {
    "high_jitter":                  "Fixed-Timeout",
    "high_jitter_adaptive":         "EWMA-Adaptive",
    "high_jitter_gossip":           "Gossip",
    "high_jitter_phi_accrual":      "φ-Accrual",
    "high_jitter_adaptive_accrual": "Adaptive-Accrual",
}

# Consistent detector order and colours across all charts
DETECTOR_ORDER = [
    "Fixed-Timeout",
    "EWMA-Adaptive",
    "Gossip",
    "φ-Accrual",
    "Adaptive-Accrual",
]

DETECTOR_COLOUR = {
    "Fixed-Timeout":    "#e74c3c",
    "EWMA-Adaptive":    "#f39c12",
    "Gossip":           "#3498db",
    "φ-Accrual":        "#9b59b6",
    "Adaptive-Accrual": "#2ecc71",
}

# Scenarios to exclude from broad overview charts (tuning variants, template)
EXCLUDE_OVERVIEW = {
    "custom_example",
    "high_jitter_adaptive_accrual_t3",
    "high_jitter_adaptive_accrual_w1000",
}


# ── Data loading ───────────────────────────────────────────────────────────────

def load_summary(path: str) -> pd.DataFrame:
    df = pd.read_csv(path)
    for col in ["mean_detection_latency", "p50_latency", "p95_latency",
                "p99_latency", "wall_time_ms"]:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors="coerce")
    for col in ["false_negatives", "p50_latency", "p95_latency", "p99_latency"]:
        if col not in df.columns:
            df[col] = float("nan")
    return df


def load_sweep(path: str) -> tuple:
    """Load a sweep CSV produced by `faultsim sweep`. Returns (param_name, df)."""
    df = pd.read_csv(path)
    for col in ["mean_detection_latency", "p50_latency", "p95_latency",
                "p99_latency", "wall_time_ms"]:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors="coerce")
    param = df.columns[0]
    return param, df


# ── Helpers ────────────────────────────────────────────────────────────────────

def _sorted_by_detector_order(df, scenario_label_map):
    """Filter df to scenarios in the map and sort by DETECTOR_ORDER."""
    df = df[df["scenario"].isin(scenario_label_map)].copy()
    df["_label"] = df["scenario"].map(scenario_label_map)
    df["_order"] = df["_label"].map({d: i for i, d in enumerate(DETECTOR_ORDER)})
    return df.sort_values("_order").reset_index(drop=True)


def _annotate_bars(ax, bars, fmt="{:.2f}", y_offset=0.012, fontsize=8):
    """Place value labels above each bar."""
    for bar in bars:
        h = bar.get_height()
        ax.text(
            bar.get_x() + bar.get_width() / 2,
            h + y_offset,
            fmt.format(h),
            ha="center", va="bottom", fontsize=fontsize,
        )


# ── Main paper figure ──────────────────────────────────────────────────────────

def plot_strategy_comparison(df: pd.DataFrame, output_dir: str):
    """
    Three-panel comparison figure intended for the paper:
      (a) False positive rate — crash scenario
      (b) Detection latency (mean + p95) — crash scenario
      (c) False positive rate — high-jitter scenario (no crashes)
    """
    crash_df  = _sorted_by_detector_order(df, CRASH_SCENARIO_LABEL)
    jitter_df = _sorted_by_detector_order(df, JITTER_SCENARIO_LABEL)

    if len(crash_df) < 2:
        print("  Skipping strategy comparison (need ≥2 crash_* scenarios)")
        return
    if len(jitter_df) < 2:
        print("  Warning: fewer than 2 high-jitter scenarios; panel (c) may be incomplete")

    fig, axes = plt.subplots(1, 3, figsize=(14, 4.8))

    # ── Panel (a): FP rate — crash scenario ───────────────────────────────────
    ax = axes[0]
    crash_labels  = crash_df["_label"].tolist()
    crash_colours = [DETECTOR_COLOUR[l] for l in crash_labels]
    bars = ax.bar(crash_labels, crash_df["false_positive_rate"],
                  color=crash_colours, edgecolor="black", linewidth=0.6, zorder=3)
    _annotate_bars(ax, bars, fmt="{:.2f}", y_offset=0.012)
    ax.set_title("(a) False Positive Rate\n(crash scenario)", pad=6)
    ax.set_ylabel("Rate")
    ax.set_ylim(0, 0.75)
    ax.yaxis.grid(True, linewidth=0.4, alpha=0.6, zorder=0)
    ax.set_axisbelow(True)
    ax.tick_params(axis="x", rotation=30)
    for tick in ax.get_xticklabels():
        tick.set_ha("right")

    # ── Panel (b): Detection latency — crash scenario ─────────────────────────
    ax = axes[1]
    x = range(len(crash_labels))
    w = 0.38
    b_mean = ax.bar([i - w / 2 for i in x],
                    crash_df["mean_detection_latency"].fillna(0),
                    width=w, label="mean", color="#4a90d9",
                    edgecolor="black", linewidth=0.6, zorder=3)
    b_p95  = ax.bar([i + w / 2 for i in x],
                    crash_df["p95_latency"].fillna(0),
                    width=w, label="p95",  color="#e67e22",
                    edgecolor="black", linewidth=0.6, zorder=3)
    ax.set_title("(b) Detection Latency\n(crash scenario, ticks)", pad=6)
    ax.set_ylabel("Ticks")
    ax.set_xticks(list(x))
    ax.set_xticklabels(crash_labels, rotation=30, ha="right")
    ax.legend(loc="upper left", framealpha=0.9, edgecolor="0.8")
    ax.yaxis.grid(True, linewidth=0.4, alpha=0.6, zorder=0)
    ax.set_axisbelow(True)

    # ── Panel (c): FP rate — high-jitter scenario ─────────────────────────────
    ax = axes[2]
    if not jitter_df.empty:
        jitter_labels  = jitter_df["_label"].tolist()
        jitter_colours = [DETECTOR_COLOUR[l] for l in jitter_labels]
        bars = ax.bar(jitter_labels, jitter_df["false_positive_rate"],
                      color=jitter_colours, edgecolor="black", linewidth=0.6, zorder=3)
        _annotate_bars(ax, bars, fmt="{:.2f}", y_offset=0.012)
    ax.set_title("(c) False Positive Rate\n(high-jitter, no crashes)", pad=6)
    ax.set_ylabel("Rate")
    ax.set_ylim(0, 1.25)
    ax.yaxis.grid(True, linewidth=0.4, alpha=0.6, zorder=0)
    ax.set_axisbelow(True)
    ax.tick_params(axis="x", rotation=30)
    for tick in ax.get_xticklabels():
        tick.set_ha("right")

    fig.suptitle("FaultSim: Detector Strategy Comparison", fontsize=12, y=1.02)
    plt.tight_layout()
    path = os.path.join(output_dir, "strategy_comparison.png")
    plt.savefig(path, dpi=300, bbox_inches="tight")
    plt.close()
    print(f"  Saved {path}")


# ── Supporting figures ─────────────────────────────────────────────────────────

def plot_detection_latency(df: pd.DataFrame, output_dir: str):
    """Mean and p95 detection latency for the five canonical crash scenarios."""
    crash_df = _sorted_by_detector_order(df, CRASH_SCENARIO_LABEL)
    crash_df = crash_df.dropna(subset=["mean_detection_latency"])
    if crash_df.empty:
        print("  Skipping latency plot (no crash scenarios with detections)")
        return

    labels  = crash_df["_label"].tolist()
    x = range(len(labels))
    w = 0.38

    fig, ax = plt.subplots(figsize=(8, 5))
    ax.bar([i - w / 2 for i in x], crash_df["mean_detection_latency"],
           width=w, label="mean", color="#4a90d9",
           edgecolor="black", linewidth=0.6, zorder=3)
    ax.bar([i + w / 2 for i in x], crash_df["p95_latency"].fillna(0),
           width=w, label="p95",  color="#e67e22",
           edgecolor="black", linewidth=0.6, zorder=3)
    ax.set_ylabel("Detection Latency (ticks)")
    ax.set_title("Detection Latency by Detector Strategy (crash scenario)")
    ax.set_xticks(list(x))
    ax.set_xticklabels(labels, rotation=20, ha="right")
    ax.legend(framealpha=0.9, edgecolor="0.8")
    ax.yaxis.grid(True, linewidth=0.4, alpha=0.6, zorder=0)
    ax.set_axisbelow(True)
    plt.tight_layout()
    path = os.path.join(output_dir, "latency.png")
    plt.savefig(path, dpi=300)
    plt.close()
    print(f"  Saved {path}")


def plot_fp_rate(df: pd.DataFrame, output_dir: str):
    """FP rate across scenarios, excluding tuning variants and the custom template."""
    plot_df = df[~df["scenario"].isin(EXCLUDE_OVERVIEW)].copy()

    fig, ax = plt.subplots(figsize=(max(9, len(plot_df) * 0.6), 5))
    colours = [
        "#2ecc71" if v == 0 else "#e74c3c" if v >= 1.0 else "#f39c12"
        for v in plot_df["false_positive_rate"]
    ]
    bars = ax.bar(plot_df["scenario"], plot_df["false_positive_rate"],
                  color=colours, edgecolor="black", linewidth=0.5, zorder=3)
    _annotate_bars(ax, bars, fmt="{:.2f}", y_offset=0.012)
    ax.set_ylabel("False Positive Rate")
    ax.set_title("False Positive Rate by Scenario")
    ax.set_ylim(0, 1.22)
    ax.yaxis.grid(True, linewidth=0.4, alpha=0.6, zorder=0)
    ax.set_axisbelow(True)
    plt.xticks(rotation=40, ha="right")
    plt.tight_layout()
    path = os.path.join(output_dir, "fp_rate.png")
    plt.savefig(path, dpi=300)
    plt.close()
    print(f"  Saved {path}")


def plot_wall_time(df: pd.DataFrame, output_dir: str):
    """Horizontal bar chart of wall-clock run time per scenario (float ms precision)."""
    if "wall_time_ms" not in df.columns:
        print("  Skipping wall-time plot (no wall_time_ms column)")
        return

    t = df[["scenario", "wall_time_ms"]].copy()
    t["wall_time_ms"] = pd.to_numeric(t["wall_time_ms"], errors="coerce")
    t = t.dropna(subset=["wall_time_ms"]).sort_values("wall_time_ms").reset_index(drop=True)

    max_ms = t["wall_time_ms"].max()

    fig, ax = plt.subplots(figsize=(8, max(4, len(t) * 0.38)))
    ax.barh(t["scenario"], t["wall_time_ms"],
            color="#3498db", edgecolor="black", linewidth=0.5, zorder=3)

    for _, row in t.iterrows():
        ms = row["wall_time_ms"]
        label = f"{ms:.2f}" if ms >= 0.05 else "<0.05"
        ax.text(ms + max_ms * 0.02, t[t["scenario"] == row["scenario"]].index[0],
                label, va="center", fontsize=8)

    ax.set_xlabel("Wall-clock time (ms)")
    ax.set_title("Simulation Run Time per Scenario")
    ax.set_xlim(0, max_ms * 1.3 + 0.1)
    ax.xaxis.grid(True, linewidth=0.4, alpha=0.6, zorder=0)
    ax.set_axisbelow(True)
    plt.tight_layout()
    path = os.path.join(output_dir, "wall_time.png")
    plt.savefig(path, dpi=300)
    plt.close()
    print(f"  Saved {path}")


def plot_sweep(param: str, df: pd.DataFrame, output_dir: str):
    """2×2 line plots of all metrics against a swept parameter."""
    fig, axes = plt.subplots(2, 2, figsize=(11, 8))
    fig.suptitle(f"Parameter Sweep: {param}", fontsize=12)

    x = df[param]

    axes[0, 0].plot(x, df["false_positive_rate"], "o-", color="#e74c3c", linewidth=1.5)
    axes[0, 0].set_xlabel(param)
    axes[0, 0].set_ylabel("False Positive Rate")
    axes[0, 0].set_title("FP Rate")
    axes[0, 0].set_ylim(-0.05, 1.1)
    axes[0, 0].yaxis.grid(True, linewidth=0.4, alpha=0.6)

    axes[0, 1].plot(x, df["false_negatives"].fillna(0), "o-", color="#e67e22", linewidth=1.5)
    axes[0, 1].set_xlabel(param)
    axes[0, 1].set_ylabel("Count")
    axes[0, 1].set_title("False Negatives")
    axes[0, 1].yaxis.grid(True, linewidth=0.4, alpha=0.6)

    lat = df["mean_detection_latency"]
    axes[1, 0].plot(x, lat, "o-", color="#4a90d9", linewidth=1.5, label="mean")
    if "p95_latency" in df.columns:
        axes[1, 0].plot(x, df["p95_latency"], "s--", color="#e74c3c",
                        linewidth=1, label="p95", markersize=4)
    axes[1, 0].set_xlabel(param)
    axes[1, 0].set_ylabel("Ticks")
    axes[1, 0].set_title("Detection Latency")
    axes[1, 0].legend(framealpha=0.9)
    axes[1, 0].yaxis.grid(True, linewidth=0.4, alpha=0.6)

    axes[1, 1].plot(x, df["message_count"], "o-", color="#9b59b6", linewidth=1.5)
    axes[1, 1].set_xlabel(param)
    axes[1, 1].set_ylabel("Messages")
    axes[1, 1].set_title("Total Messages")
    axes[1, 1].yaxis.grid(True, linewidth=0.4, alpha=0.6)

    plt.tight_layout()
    safe_param = param.replace("/", "_")
    path = os.path.join(output_dir, f"sweep_{safe_param}.png")
    plt.savefig(path, dpi=300)
    plt.close()
    print(f"  Saved {path}")


# ── Entry point ────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Plot faultsim results")
    parser.add_argument("csv", help="Path to summary.csv or a sweep CSV")
    parser.add_argument("--output", "-o", default="results/figures",
                        help="Output directory for plot images (default: results/figures)")
    parser.add_argument("--sweep", metavar="PARAM",
                        help="Treat CSV as a sweep file with PARAM as the x-axis column")
    args = parser.parse_args()

    os.makedirs(args.output, exist_ok=True)

    if args.sweep:
        param, df = load_sweep(args.csv)
        print(f"Sweep plot: {param} ({len(df)} steps)")
        plot_sweep(param, df, args.output)
    else:
        df = load_summary(args.csv)
        df = df.sort_values("scenario").reset_index(drop=True)
        print(f"Loaded {len(df)} scenario results. Generating plots...")
        plot_fp_rate(df, args.output)
        plot_detection_latency(df, args.output)
        plot_strategy_comparison(df, args.output)
        plot_wall_time(df, args.output)

    print(f"\nPlots saved to {args.output}/")


if __name__ == "__main__":
    main()
