#!/usr/bin/env python3
"""
Plot the φ (accrual suspicion) timeline for a single simulation run.

This produces the canonical figure from Hayashibara et al. (SRDS 2004):
φ value vs. simulation tick, with crash and recovery events marked as
vertical lines and the suspicion threshold as a horizontal dashed line.

Usage:
    python scripts/plot_phi_timeline.py \\
        results/phi/crash_phi_accrual_phi_log.csv \\
        results/phi/crash_phi_accrual_events.csv \\
        --threshold 8.0 \\
        --output results/figures/phi_timeline.png

Arguments:
    phi_log     CSV with columns: tick, observer, observed, phi
    events      CSV with columns: tick, kind, node  (kind = crash | recovery)

Options:
    --threshold FLOAT   Suspicion threshold line (default: 8.0)
    --observer INT      Only show φ from this observer node (default: all)
    --observed  INT     Only show φ for this observed node (default: all)
    --cap       FLOAT   Cap φ at this value for readability (default: 20)
    --output    PATH    Output image path (default: phi_timeline.png)
    --title     TEXT    Custom plot title

Requires: pip install matplotlib pandas
"""

import argparse
import os
import sys

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import matplotlib.lines as mlines
    import pandas as pd
except ImportError:
    print("Error: matplotlib and pandas are required.")
    print("  pip install matplotlib pandas")
    sys.exit(1)


# ── Colour helpers ─────────────────────────────────────────────────────────────

# A qualitative palette that works in greyscale print.
_PALETTE = [
    "#3498db", "#e67e22", "#2ecc71", "#9b59b6",
    "#1abc9c", "#e74c3c", "#34495e", "#f1c40f",
]


def _colour(i: int) -> str:
    return _PALETTE[i % len(_PALETTE)]


# ── Main ───────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Plot φ accrual suspicion timeline",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__.split("Requires:")[0],
    )
    parser.add_argument("phi_log", help="phi_log CSV (tick,observer,observed,phi)")
    parser.add_argument("events",  help="events CSV (tick,kind,node)")
    parser.add_argument("--threshold", type=float, default=8.0,
                        help="Suspicion threshold (default: 8.0)")
    parser.add_argument("--observer", type=int, default=None,
                        help="Filter to one observer node")
    parser.add_argument("--observed", type=int, default=None,
                        help="Filter to one observed node")
    parser.add_argument("--cap", type=float, default=20.0,
                        help="Cap φ at this value for plot readability (default: 20)")
    parser.add_argument("--output", "-o", default="phi_timeline.png",
                        help="Output image path (default: phi_timeline.png)")
    parser.add_argument("--title", default=None,
                        help="Custom plot title")
    args = parser.parse_args()

    # ── Load data ──────────────────────────────────────────────────────────────

    phi_df = pd.read_csv(args.phi_log)
    # 'inf' values were written as the string 'inf'; pandas reads them as float inf.
    phi_df["phi"] = pd.to_numeric(phi_df["phi"], errors="coerce").clip(upper=args.cap)

    events_df = pd.read_csv(args.events)

    # ── Filter ────────────────────────────────────────────────────────────────

    if args.observer is not None:
        phi_df = phi_df[phi_df["observer"] == args.observer]
    if args.observed is not None:
        phi_df = phi_df[phi_df["observed"] == args.observed]

    if phi_df.empty:
        print("Error: no φ data after filtering. Check --observer / --observed values.")
        print(f"  Available observers: {sorted(pd.read_csv(args.phi_log)['observer'].unique())}")
        print(f"  Available observed:  {sorted(pd.read_csv(args.phi_log)['observed'].unique())}")
        sys.exit(1)

    # ── Plot ──────────────────────────────────────────────────────────────────

    fig, ax = plt.subplots(figsize=(14, 6))

    groups = list(phi_df.groupby(["observer", "observed"]))
    for idx, ((observer, observed), grp) in enumerate(groups):
        grp = grp.sort_values("tick")
        alpha = 0.75 if len(groups) <= 5 else max(0.2, 0.75 - idx * 0.05)
        lw = 1.2 if len(groups) <= 5 else 0.7
        ax.plot(
            grp["tick"], grp["phi"],
            linewidth=lw, alpha=alpha,
            color=_colour(idx),
            label=f"n{observer}→n{observed}",
        )

    # ── Event markers ─────────────────────────────────────────────────────────

    crash_nodes_seen = set()
    recovery_nodes_seen = set()

    for _, row in events_df.iterrows():
        tick = int(row["tick"])
        node = int(row["node"])
        kind = str(row["kind"])
        if kind == "crash":
            label = f"crash n{node}" if node not in crash_nodes_seen else None
            crash_nodes_seen.add(node)
            ax.axvline(tick, color="#e74c3c", linestyle="--", linewidth=1.5,
                       alpha=0.9, label=label)
            ax.text(tick, args.cap * 0.97, f"↓n{node}",
                    color="#e74c3c", fontsize=7, ha="left", va="top")
        elif kind == "recovery":
            label = f"recovery n{node}" if node not in recovery_nodes_seen else None
            recovery_nodes_seen.add(node)
            ax.axvline(tick, color="#2ecc71", linestyle=":", linewidth=1.5,
                       alpha=0.9, label=label)
            ax.text(tick, args.cap * 0.97, f"↑n{node}",
                    color="#2ecc71", fontsize=7, ha="left", va="top")

    # ── Threshold line ────────────────────────────────────────────────────────

    ax.axhline(
        args.threshold, color="#f39c12", linestyle="-.", linewidth=1.5,
        label=f"threshold = {args.threshold}",
    )

    # ── Labels + legend ───────────────────────────────────────────────────────

    ax.set_xlabel("Simulation Tick", fontsize=11)
    ax.set_ylabel("φ  (accrual suspicion level)", fontsize=11)

    title = args.title or "φ Accrual Failure Detector — Suspicion Timeline"
    ax.set_title(title, fontsize=13)

    ax.set_ylim(bottom=-0.2)
    ax.set_xlim(left=phi_df["tick"].min())

    # Deduplicate legend labels (crash/recovery lines add label only once).
    handles, labels = ax.get_legend_handles_labels()
    by_label = dict(zip(labels, handles))
    n_series = len(groups)
    ncol = max(1, min(4, (len(by_label) + 2) // 3))
    ax.legend(
        by_label.values(), by_label.keys(),
        loc="upper left", fontsize=7, ncol=ncol,
        framealpha=0.8,
    )

    plt.tight_layout()

    out = args.output
    os.makedirs(os.path.dirname(out) if os.path.dirname(out) else ".", exist_ok=True)
    fig.savefig(out, dpi=150)
    plt.close()
    print(f"Saved: {out}")
    print(f"  Series plotted: {n_series}")
    print(f"  Tick range:     {phi_df['tick'].min()} – {phi_df['tick'].max()}")
    print(f"  φ range:        {phi_df['phi'].min():.2f} – {phi_df['phi'].max():.2f} (capped at {args.cap})")


if __name__ == "__main__":
    main()
