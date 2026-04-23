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
from pathlib import Path

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import pandas as pd
except ImportError:
    print("Error: matplotlib and pandas are required.")
    print("  pip install matplotlib pandas")
    sys.exit(1)


# ── Colour palette ─────────────────────────────────────────────────────────────

DETECTOR_COLOURS = {
    "fixed_timeout":      "#e74c3c",
    "adaptive":           "#f39c12",
    "gossip":             "#3498db",
    "phi_accrual":        "#9b59b6",
    "adaptive_accrual":   "#2ecc71",
}

# Map scenario names to detector labels for the strategy comparison chart.
DETECTOR_FOR_SCENARIO = {
    "crash_recovery":          "FixedTimeout",
    "crash_adaptive":          "Adaptive",
    "crash_gossip":            "Gossip",
    "crash_phi_accrual":       "φ-Accrual",
    "crash_adaptive_accrual":  "Adaptive Accrual",
}


# ── Data loading ───────────────────────────────────────────────────────────────

def load_summary(path: str) -> pd.DataFrame:
    df = pd.read_csv(path)
    # Coerce N/A latency strings to NaN so pandas treats them as missing.
    for col in ["mean_detection_latency", "p50_latency", "p95_latency", "p99_latency"]:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors="coerce")
    # Handle older summary files that lack the new columns.
    for col in ["false_negatives", "p50_latency", "p95_latency", "p99_latency"]:
        if col not in df.columns:
            df[col] = float("nan")
    return df


def load_sweep(path: str) -> tuple[str, pd.DataFrame]:
    """Load a sweep CSV produced by `faultsim sweep`. Returns (param_name, df)."""
    df = pd.read_csv(path)
    for col in ["mean_detection_latency", "p50_latency", "p95_latency", "p99_latency"]:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors="coerce")
    # First column is the swept parameter.
    param = df.columns[0]
    return param, df


# ── Individual plots ───────────────────────────────────────────────────────────

def _bar_label(ax, bars, fmt="{:.2f}", offset=0.01):
    for bar in bars:
        h = bar.get_height()
        ax.text(
            bar.get_x() + bar.get_width() / 2,
            h + offset,
            fmt.format(h),
            ha="center", va="bottom", fontsize=7,
        )


def plot_fp_rate(df: pd.DataFrame, output_dir: str):
    fig, ax = plt.subplots(figsize=(max(8, len(df) * 0.65), 5))
    colours = [
        "#2ecc71" if v == 0 else "#e74c3c" if v >= 1.0 else "#f39c12"
        for v in df["false_positive_rate"]
    ]
    bars = ax.bar(df["scenario"], df["false_positive_rate"], color=colours,
                  edgecolor="black", linewidth=0.5)
    _bar_label(ax, bars)
    ax.set_ylabel("False Positive Rate")
    ax.set_title("False Positive Rate by Scenario")
    ax.set_ylim(0, 1.15)
    plt.xticks(rotation=45, ha="right")
    plt.tight_layout()
    path = os.path.join(output_dir, "fp_rate.png")
    plt.savefig(path, dpi=150)
    plt.close()
    print(f"  Saved {path}")


def plot_false_negatives(df: pd.DataFrame, output_dir: str):
    fn_df = df[df["false_negatives"].notna() & (df["false_negatives"] > 0)]
    if fn_df.empty:
        # Still produce the chart — all zeros is a meaningful result.
        fn_df = df.copy()
        fn_df["false_negatives"] = fn_df["false_negatives"].fillna(0)

    fig, ax = plt.subplots(figsize=(max(8, len(fn_df) * 0.65), 5))
    colours = [
        "#e74c3c" if v > 0 else "#2ecc71"
        for v in fn_df["false_negatives"]
    ]
    bars = ax.bar(fn_df["scenario"], fn_df["false_negatives"], color=colours,
                  edgecolor="black", linewidth=0.5)
    _bar_label(ax, bars, fmt="{:.0f}", offset=0.05)
    ax.set_ylabel("Undetected Crashes (False Negatives)")
    ax.set_title("False Negatives by Scenario")
    plt.xticks(rotation=45, ha="right")
    plt.tight_layout()
    path = os.path.join(output_dir, "false_negatives.png")
    plt.savefig(path, dpi=150)
    plt.close()
    print(f"  Saved {path}")


def plot_detection_latency(df: pd.DataFrame, output_dir: str):
    crash_df = df.dropna(subset=["mean_detection_latency"])
    if crash_df.empty:
        print("  Skipping latency plot (no crash scenarios with detections)")
        return

    fig, ax = plt.subplots(figsize=(max(6, len(crash_df) * 0.9), 5))
    x = range(len(crash_df))
    w = 0.25

    ax.bar([i - w for i in x], crash_df["mean_detection_latency"].fillna(0),
           width=w, label="mean", color="#3498db", edgecolor="black", linewidth=0.5)
    ax.bar([i for i in x], crash_df["p95_latency"].fillna(0),
           width=w, label="p95",  color="#e67e22", edgecolor="black", linewidth=0.5)
    ax.bar([i + w for i in x], crash_df["p99_latency"].fillna(0),
           width=w, label="p99",  color="#e74c3c", edgecolor="black", linewidth=0.5)

    ax.set_ylabel("Detection Latency (ticks)")
    ax.set_title("Detection Latency — mean / p95 / p99 (crash scenarios)")
    ax.set_xticks(list(x))
    ax.set_xticklabels(crash_df["scenario"].tolist(), rotation=45, ha="right")
    ax.legend()
    plt.tight_layout()
    path = os.path.join(output_dir, "latency.png")
    plt.savefig(path, dpi=150)
    plt.close()
    print(f"  Saved {path}")


def plot_strategy_comparison(df: pd.DataFrame, output_dir: str):
    """Side-by-side comparison of all five detectors on their crash scenarios."""
    strat_df = df[df["scenario"].isin(DETECTOR_FOR_SCENARIO)].copy()
    if len(strat_df) < 2:
        print("  Skipping strategy comparison (need ≥2 crash_* scenarios)")
        return

    order = list(DETECTOR_FOR_SCENARIO.keys())
    strat_df["_order"] = strat_df["scenario"].map(
        {s: i for i, s in enumerate(order)}
    )
    strat_df = strat_df.sort_values("_order")
    labels = [DETECTOR_FOR_SCENARIO[s] for s in strat_df["scenario"]]

    fig, axes = plt.subplots(1, 3, figsize=(15, 5))

    # FP rate
    colours = [
        "#2ecc71" if v < 0.1 else "#f39c12" if v < 0.5 else "#e74c3c"
        for v in strat_df["false_positive_rate"]
    ]
    bars = axes[0].bar(labels, strat_df["false_positive_rate"], color=colours,
                       edgecolor="black", linewidth=0.5)
    _bar_label(axes[0], bars)
    axes[0].set_title("False Positive Rate")
    axes[0].set_ylim(0, 1.15)

    # Latency (mean + p95 grouped)
    x = range(len(labels))
    w = 0.35
    axes[1].bar([i - w/2 for i in x], strat_df["mean_detection_latency"].fillna(0),
                width=w, label="mean", color="#3498db", edgecolor="black", linewidth=0.5)
    axes[1].bar([i + w/2 for i in x], strat_df["p95_latency"].fillna(0),
                width=w, label="p95",  color="#e67e22", edgecolor="black", linewidth=0.5)
    axes[1].set_title("Detection Latency (ticks)")
    axes[1].set_xticks(list(x))
    axes[1].set_xticklabels(labels)
    axes[1].legend(fontsize=8)

    # False negatives
    fn_vals = strat_df["false_negatives"].fillna(0)
    fn_colours = ["#e74c3c" if v > 0 else "#2ecc71" for v in fn_vals]
    bars = axes[2].bar(labels, fn_vals, color=fn_colours,
                       edgecolor="black", linewidth=0.5)
    _bar_label(axes[2], bars, fmt="{:.0f}", offset=0.05)
    axes[2].set_title("False Negatives")

    fig.suptitle("Detector Strategy Comparison (crash scenarios)", fontsize=13)
    plt.tight_layout()
    path = os.path.join(output_dir, "strategy_comparison.png")
    plt.savefig(path, dpi=150, bbox_inches="tight")
    plt.close()
    print(f"  Saved {path}")


def plot_sweep(param: str, df: pd.DataFrame, output_dir: str):
    """Line plots of all metrics against a swept parameter."""
    fig, axes = plt.subplots(2, 2, figsize=(12, 9))
    fig.suptitle(f"Parameter Sweep: {param}", fontsize=13)

    x = df[param]

    # FP rate
    axes[0, 0].plot(x, df["false_positive_rate"], "o-", color="#e74c3c", linewidth=1.5)
    axes[0, 0].set_xlabel(param)
    axes[0, 0].set_ylabel("False Positive Rate")
    axes[0, 0].set_title("FP Rate")
    axes[0, 0].set_ylim(-0.05, 1.1)

    # False negatives
    axes[0, 1].plot(x, df["false_negatives"].fillna(0), "o-", color="#e67e22", linewidth=1.5)
    axes[0, 1].set_xlabel(param)
    axes[0, 1].set_ylabel("Count")
    axes[0, 1].set_title("False Negatives")

    # Mean latency
    lat = df["mean_detection_latency"]
    axes[1, 0].plot(x, lat, "o-", color="#3498db", linewidth=1.5, label="mean")
    if "p95_latency" in df.columns:
        axes[1, 0].plot(x, df["p95_latency"], "s--", color="#e74c3c",
                        linewidth=1, label="p95", markersize=4)
    axes[1, 0].set_xlabel(param)
    axes[1, 0].set_ylabel("Ticks")
    axes[1, 0].set_title("Detection Latency")
    axes[1, 0].legend(fontsize=8)

    # Message overhead
    axes[1, 1].plot(x, df["message_count"], "o-", color="#9b59b6", linewidth=1.5)
    axes[1, 1].set_xlabel(param)
    axes[1, 1].set_ylabel("Messages")
    axes[1, 1].set_title("Total Messages")

    plt.tight_layout()
    safe_param = param.replace("/", "_")
    path = os.path.join(output_dir, f"sweep_{safe_param}.png")
    plt.savefig(path, dpi=150)
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
        df = df.sort_values("scenario")
        print(f"Loaded {len(df)} scenario results. Generating plots...")
        plot_fp_rate(df, args.output)
        plot_false_negatives(df, args.output)
        plot_detection_latency(df, args.output)
        plot_strategy_comparison(df, args.output)

    print(f"\nPlots saved to {args.output}/")


if __name__ == "__main__":
    main()
