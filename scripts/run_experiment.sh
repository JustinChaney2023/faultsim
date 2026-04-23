#!/usr/bin/env bash
# Run a full experiment suite: batch run → seed sweep → parameter sweeps → plots.
#
# Usage:
#   ./scripts/run_experiment.sh                   # full suite, default dirs
#   ./scripts/run_experiment.sh --seeds 30        # more seeds for CI
#   ./scripts/run_experiment.sh --skip-plots      # CI mode, no Python dep
#
# Outputs:
#   results/all/         — one run per scenario (summary.csv + per-scenario detections)
#   results/seeds/       — 30-seed aggregation for crash scenarios
#   results/sweep/       — parameter sensitivity CSVs
#   results/figures/     — PNG plots (requires matplotlib + pandas)

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────────

SEEDS=30
SKIP_PLOTS=false
SCENARIO_DIR="configs/scenarios"
RESULTS_DIR="results"

while [[ $# -gt 0 ]]; do
    case $1 in
        --seeds)     SEEDS="$2";  shift 2 ;;
        --skip-plots) SKIP_PLOTS=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Build ─────────────────────────────────────────────────────────────────────

echo "=== Building release binary ==="
cargo build --release 2>&1
BINARY="./target/release/faultsim"

# ── Step 1: Batch run all scenarios ──────────────────────────────────────────

echo ""
echo "=== Step 1: Batch run (all scenarios) ==="
"$BINARY" run-all \
    --scenarios "$SCENARIO_DIR" \
    --output "$RESULTS_DIR/all"

# ── Step 2: Multi-seed aggregation for crash scenarios ────────────────────────

echo ""
echo "=== Step 2: Seed sweep ($SEEDS seeds, crash scenarios) ==="
mkdir -p "$RESULTS_DIR/seeds"

for scenario in crash_adaptive crash_gossip crash_phi_accrual crash_adaptive_accrual crash_recovery; do
    cfg="$SCENARIO_DIR/${scenario}.toml"
    if [[ -f "$cfg" ]]; then
        echo "  $scenario"
        "$BINARY" sweep-seeds \
            --config "$cfg" \
            --seeds "$SEEDS" \
            --output "$RESULTS_DIR/seeds"
    fi
done

# ── Step 3: Parameter sweeps ──────────────────────────────────────────────────

echo ""
echo "=== Step 3: Parameter sweeps ==="
mkdir -p "$RESULTS_DIR/sweep"

# φ threshold sensitivity (phi_accrual on crash scenario)
if [[ -f "$SCENARIO_DIR/crash_phi_accrual.toml" ]]; then
    echo "  phi_threshold sweep"
    "$BINARY" sweep \
        --config "$SCENARIO_DIR/crash_phi_accrual.toml" \
        --param phi_threshold \
        --start 2.0 --end 20.0 --steps 10 \
        --output "$RESULTS_DIR/sweep"
fi

# Packet drop sensitivity (fixed_timeout on crash scenario)
if [[ -f "$SCENARIO_DIR/crash_recovery.toml" ]]; then
    echo "  drop_probability sweep"
    "$BINARY" sweep \
        --config "$SCENARIO_DIR/crash_recovery.toml" \
        --param drop_probability \
        --start 0.0 --end 0.3 --steps 10 \
        --output "$RESULTS_DIR/sweep"
fi

# EWMA alpha sensitivity (adaptive on crash scenario)
if [[ -f "$SCENARIO_DIR/crash_adaptive.toml" ]]; then
    echo "  alpha sweep"
    "$BINARY" sweep \
        --config "$SCENARIO_DIR/crash_adaptive.toml" \
        --param alpha \
        --start 0.1 --end 0.9 --steps 9 \
        --output "$RESULTS_DIR/sweep"
fi

# ── Step 4: φ timeline logging ────────────────────────────────────────────────

echo ""
echo "=== Step 4: φ timeline log (phi_accrual crash scenario) ==="
mkdir -p "$RESULTS_DIR/phi"

# Run with phi_log enabled via a temporary TOML override.
# We patch the scenario inline by appending the output section.
TMPTOML=$(mktemp /tmp/faultsim_phi_XXXX.toml)
cat "$SCENARIO_DIR/crash_phi_accrual.toml" > "$TMPTOML"
cat >> "$TMPTOML" <<'EOF'

[output]
dir = "results/phi"
phi_log = true
EOF

"$BINARY" run --config "$TMPTOML"
rm -f "$TMPTOML"

# ── Step 5: Plots ─────────────────────────────────────────────────────────────

if [[ "$SKIP_PLOTS" == "true" ]]; then
    echo ""
    echo "=== Skipping plots (--skip-plots) ==="
else
    echo ""
    echo "=== Step 5: Plots ==="
    mkdir -p "$RESULTS_DIR/figures"

    if ! python3 -c "import matplotlib, pandas" 2>/dev/null; then
        echo "  WARNING: matplotlib/pandas not installed — skipping plots."
        echo "  Install with: pip install matplotlib pandas"
    else
        # Comparison charts
        python3 scripts/plot_results.py \
            "$RESULTS_DIR/all/summary.csv" \
            --output "$RESULTS_DIR/figures"

        # Sweep charts
        for csv in "$RESULTS_DIR/sweep"/*.csv; do
            param=$(basename "$csv" .csv | sed 's/.*_//')
            python3 scripts/plot_results.py \
                "$csv" \
                --sweep "$param" \
                --output "$RESULTS_DIR/figures"
        done

        # φ timeline (if log files exist)
        PHI_LOG=$(ls "$RESULTS_DIR/phi/"*_phi_log.csv 2>/dev/null | head -1)
        PHI_EVENTS=$(ls "$RESULTS_DIR/phi/"*_events.csv 2>/dev/null | head -1)
        if [[ -n "$PHI_LOG" && -n "$PHI_EVENTS" ]]; then
            python3 scripts/plot_phi_timeline.py \
                "$PHI_LOG" "$PHI_EVENTS" \
                --threshold 8.0 \
                --output "$RESULTS_DIR/figures/phi_timeline.png"
        fi
    fi
fi

echo ""
echo "=== Done ==="
echo "  Batch results:   $RESULTS_DIR/all/summary.csv"
echo "  Seed aggregates: $RESULTS_DIR/seeds/"
echo "  Sweep data:      $RESULTS_DIR/sweep/"
echo "  φ log:           $RESULTS_DIR/phi/"
echo "  Figures:         $RESULTS_DIR/figures/"
