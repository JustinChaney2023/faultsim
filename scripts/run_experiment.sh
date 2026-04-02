#!/usr/bin/env bash
# Run a batch of experiments across scenario configs.
#
# Usage:
#   ./scripts/run_experiment.sh configs/scenarios/*.toml
#
# Each scenario is run once. Results are written to results/<scenario_name>_<timestamp>.

set -euo pipefail

BINARY="./target/release/faultsim"

if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    cargo build --release
fi

TIMESTAMP=$(date +%Y%m%d_%H%M%S)

for config in "$@"; do
    name=$(basename "$config" .toml)
    outdir="results/${name}_${TIMESTAMP}"
    mkdir -p "$outdir"

    echo "Running scenario: $name"
    $BINARY --config "$config" > "$outdir/output.txt" 2>&1

    # Copy config for reproducibility
    cp "$config" "$outdir/config.toml"

    echo "  -> $outdir"
done

echo "Done. Results in results/"

# TODO: Add support for multiple seeds per scenario
# TODO: Add CSV aggregation step
# TODO: Add plotting step (call a Python script or gnuplot)
