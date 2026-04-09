#!/usr/bin/env python3
"""
Plot faultsim experiment results from CSV summary files.

Usage:
    python scripts/plot_results.py results/all/*/summary.csv
    python scripts/plot_results.py results/all/*/summary.csv --output results/plots

Reads summary.csv files exported by faultsim and produces comparison charts.
Requires matplotlib: pip install matplotlib
"""

import argparse
import csv
import os
import sys
from pathlib import Path

try:
    import matplotlib

    matplotlib.use("Agg")  # Non-interactive backend for headless use.
    import matplotlib.pyplot as plt
except ImportError:
    print("Error: matplotlib is required. Install with: pip install matplotlib")
    sys.exit(1)


def load_summaries(csv_paths):
    """Load summary rows from one or more CSV files."""
    rows = []
    for path in csv_paths:
        with open(path, newline="") as f:
            reader = csv.DictReader(f)
            for row in reader:
                row["messages"] = int(row["messages"])
                row["total_ticks"] = int(row["total_ticks"])
                row["detections"] = int(row["detections"])
                row["crashes"] = int(row["crashes"])
                row["recoveries"] = int(row["recoveries"])
                row["false_positive_rate"] = float(row["false_positive_rate"])
                row["messages_per_tick"] = float(row["messages_per_tick"])
                lat = row["mean_detection_latency"]
                row["mean_detection_latency"] = float(lat) if lat != "N/A" else None
                rows.append(row)
    return rows


def plot_false_positive_comparison(rows, output_dir):
    """Bar chart comparing false positive rates across scenarios."""
    scenarios = [r["scenario"] for r in rows]
    fp_rates = [r["false_positive_rate"] for r in rows]

    fig, ax = plt.subplots(figsize=(12, 5))
    colors = ["#2ecc71" if fp == 0 else "#e74c3c" if fp == 1.0 else "#f39c12" for fp in fp_rates]
    bars = ax.bar(scenarios, fp_rates, color=colors, edgecolor="black", linewidth=0.5)

    ax.set_ylabel("False Positive Rate")
    ax.set_title("False Positive Rate by Scenario")
    ax.set_ylim(0, 1.1)
    ax.axhline(y=0, color="gray", linewidth=0.5)

    # Add value labels on bars.
    for bar, val in zip(bars, fp_rates):
        ax.text(
            bar.get_x() + bar.get_width() / 2,
            bar.get_height() + 0.02,
            f"{val:.2f}",
            ha="center",
            va="bottom",
            fontsize=8,
        )

    plt.xticks(rotation=45, ha="right")
    plt.tight_layout()
    path = os.path.join(output_dir, "false_positive_rates.png")
    plt.savefig(path, dpi=150)
    plt.close()
    print(f"  Saved {path}")


def plot_detection_latency_comparison(rows, output_dir):
    """Bar chart comparing detection latency across scenarios that have crashes."""
    crash_rows = [r for r in rows if r["mean_detection_latency"] is not None]
    if not crash_rows:
        print("  Skipping detection latency plot (no crash scenarios)")
        return

    scenarios = [r["scenario"] for r in crash_rows]
    latencies = [r["mean_detection_latency"] for r in crash_rows]

    fig, ax = plt.subplots(figsize=(8, 5))
    bars = ax.bar(scenarios, latencies, color="#3498db", edgecolor="black", linewidth=0.5)

    ax.set_ylabel("Mean Detection Latency (ticks)")
    ax.set_title("Detection Latency by Scenario (crash scenarios only)")

    for bar, val in zip(bars, latencies):
        ax.text(
            bar.get_x() + bar.get_width() / 2,
            bar.get_height() + 2,
            f"{val:.0f}",
            ha="center",
            va="bottom",
            fontsize=9,
        )

    plt.xticks(rotation=45, ha="right")
    plt.tight_layout()
    path = os.path.join(output_dir, "detection_latency.png")
    plt.savefig(path, dpi=150)
    plt.close()
    print(f"  Saved {path}")


def plot_detection_count_comparison(rows, output_dir):
    """Bar chart of detection event counts, colored by true/false positive mix."""
    scenarios = [r["scenario"] for r in rows]
    detections = [r["detections"] for r in rows]
    fp_rates = [r["false_positive_rate"] for r in rows]

    # Split into true positives and false positives.
    fps = [int(d * fp) for d, fp in zip(detections, fp_rates)]
    tps = [d - fp for d, fp in zip(detections, fps)]

    fig, ax = plt.subplots(figsize=(12, 5))
    x = range(len(scenarios))

    ax.bar(x, tps, color="#2ecc71", edgecolor="black", linewidth=0.5, label="True Positives")
    ax.bar(x, fps, bottom=tps, color="#e74c3c", edgecolor="black", linewidth=0.5, label="False Positives")

    ax.set_ylabel("Detection Events")
    ax.set_title("Detection Events by Scenario (True vs False Positives)")
    ax.set_xticks(x)
    ax.set_xticklabels(scenarios, rotation=45, ha="right")
    ax.legend()
    plt.tight_layout()
    path = os.path.join(output_dir, "detection_counts.png")
    plt.savefig(path, dpi=150)
    plt.close()
    print(f"  Saved {path}")


def plot_messages_per_tick(rows, output_dir):
    """Bar chart of messaging overhead across scenarios."""
    scenarios = [r["scenario"] for r in rows]
    mpt = [r["messages_per_tick"] for r in rows]

    fig, ax = plt.subplots(figsize=(12, 5))
    ax.bar(scenarios, mpt, color="#9b59b6", edgecolor="black", linewidth=0.5)

    ax.set_ylabel("Messages per Tick")
    ax.set_title("Messaging Overhead by Scenario")
    ax.set_ylim(0, max(mpt) * 1.15)

    for i, val in enumerate(mpt):
        ax.text(i, val + 0.01, f"{val:.2f}", ha="center", va="bottom", fontsize=8)

    plt.xticks(rotation=45, ha="right")
    plt.tight_layout()
    path = os.path.join(output_dir, "messages_per_tick.png")
    plt.savefig(path, dpi=150)
    plt.close()
    print(f"  Saved {path}")


def plot_strategy_comparison(rows, output_dir):
    """
    Grouped bar chart comparing the three detector strategies on the crash scenario.
    Looks for scenarios named crash_recovery (fixed_timeout), crash_adaptive, crash_gossip.
    """
    strategy_map = {
        "crash_recovery": "Fixed-Timeout",
        "crash_adaptive": "Adaptive",
        "crash_gossip": "Gossip",
    }
    crash_rows = [r for r in rows if r["scenario"] in strategy_map]
    if len(crash_rows) < 2:
        print("  Skipping strategy comparison (need at least 2 crash_* scenarios)")
        return

    # Sort by strategy order.
    order = ["crash_recovery", "crash_adaptive", "crash_gossip"]
    crash_rows.sort(key=lambda r: order.index(r["scenario"]) if r["scenario"] in order else 99)

    labels = [strategy_map[r["scenario"]] for r in crash_rows]
    fp_rates = [r["false_positive_rate"] for r in crash_rows]
    latencies = [r["mean_detection_latency"] or 0 for r in crash_rows]
    detections = [r["detections"] for r in crash_rows]

    fig, axes = plt.subplots(1, 3, figsize=(14, 5))

    # FP rate.
    colors = ["#2ecc71" if fp < 0.2 else "#f39c12" if fp < 0.8 else "#e74c3c" for fp in fp_rates]
    axes[0].bar(labels, fp_rates, color=colors, edgecolor="black", linewidth=0.5)
    axes[0].set_ylabel("False Positive Rate")
    axes[0].set_title("FP Rate")
    axes[0].set_ylim(0, 1.1)
    for i, v in enumerate(fp_rates):
        axes[0].text(i, v + 0.03, f"{v:.2f}", ha="center", fontsize=9)

    # Latency.
    axes[1].bar(labels, latencies, color="#3498db", edgecolor="black", linewidth=0.5)
    axes[1].set_ylabel("Ticks")
    axes[1].set_title("Detection Latency")
    for i, v in enumerate(latencies):
        axes[1].text(i, v + 3, f"{v:.0f}", ha="center", fontsize=9)

    # Detection count.
    axes[2].bar(labels, detections, color="#9b59b6", edgecolor="black", linewidth=0.5)
    axes[2].set_ylabel("Events")
    axes[2].set_title("Detection Events")
    for i, v in enumerate(detections):
        axes[2].text(i, v + 0.3, str(v), ha="center", fontsize=9)

    fig.suptitle("Strategy Comparison: Crash + Recovery Scenario", fontsize=13, y=1.02)
    plt.tight_layout()
    path = os.path.join(output_dir, "strategy_comparison.png")
    plt.savefig(path, dpi=150, bbox_inches="tight")
    plt.close()
    print(f"  Saved {path}")


def main():
    parser = argparse.ArgumentParser(description="Plot faultsim experiment results")
    parser.add_argument("csv_files", nargs="+", help="Path(s) to summary.csv files")
    parser.add_argument(
        "--output", "-o", default="results/plots", help="Output directory for plots"
    )
    args = parser.parse_args()

    rows = load_summaries(args.csv_files)
    if not rows:
        print("No data found in the provided CSV files.")
        sys.exit(1)

    # Sort by scenario name for consistent ordering.
    rows.sort(key=lambda r: r["scenario"])

    os.makedirs(args.output, exist_ok=True)
    print(f"Loaded {len(rows)} scenario results. Generating plots...")

    plot_false_positive_comparison(rows, args.output)
    plot_detection_latency_comparison(rows, args.output)
    plot_detection_count_comparison(rows, args.output)
    plot_messages_per_tick(rows, args.output)
    plot_strategy_comparison(rows, args.output)

    print(f"\nAll plots saved to {args.output}/")


if __name__ == "__main__":
    main()
