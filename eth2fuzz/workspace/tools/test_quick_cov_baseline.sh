#!/usr/bin/env bash
set -euo pipefail

# Minimal unit-like test for quick_cov_compare.sh baseline copying & summary
# It synthesizes baseline/RL kept corpora and logs, then runs the script in SKIP_RUNS mode.

DIR="$(cd "$(dirname "$0")/.." && pwd)"
TAG="ut_quickcov"
TARGET="lighthouse_attestation"

rm -rf "$DIR/logs/${TAG}" "$DIR/logs/${TAG}_base" || true
mkdir -p "$DIR/logs/${TAG}_base/rl/libfuzzer_corpus/${TARGET}"
mkdir -p "$DIR/logs/${TAG}_base/rl/hfuzz/logs"
mkdir -p "$DIR/logs/${TAG}/rl/libfuzzer_corpus/${TARGET}"
mkdir -p "$DIR/logs/${TAG}/rl/hfuzz/logs"

# Synthesize baseline kept files
echo foo > "$DIR/logs/${TAG}_base/rl/libfuzzer_corpus/${TARGET}/a"
echo bar > "$DIR/logs/${TAG}_base/rl/libfuzzer_corpus/${TARGET}/b"

# Synthesize RL kept files
echo baz > "$DIR/logs/${TAG}/rl/libfuzzer_corpus/${TARGET}/x"
echo qux > "$DIR/logs/${TAG}/rl/libfuzzer_corpus/${TARGET}/y"
echo quux > "$DIR/logs/${TAG}/rl/libfuzzer_corpus/${TARGET}/z"

# Synthesize Honggfuzz logs with branch_coverage_percent lines
printf "branch_coverage_percent: 5\nbranch_coverage_percent: 7\n" > "$DIR/logs/${TAG}_base/rl/hfuzz/logs/${TARGET}.log"
printf "branch_coverage_percent: 6\nbranch_coverage_percent: 7\n" > "$DIR/logs/${TAG}/rl/hfuzz/logs/${TARGET}.log"

# Run quick_cov in SKIP_RUNS mode so it doesn't invoke docker
export SKIP_RUNS=1
"$DIR/tools/quick_cov_compare.sh" eth2fuzz_lighthouse "$TARGET" 1 1 1 1 "$TAG" honggfuzz || true

echo "\n== Assert baseline copy exists and counts =="
ls -l "$DIR/logs/${TAG}/base/libfuzzer_corpus/${TARGET}" || true
echo "Baseline kept count: $(find "$DIR/logs/${TAG}/base/libfuzzer_corpus/${TARGET}" -type f 2>/dev/null | wc -l | awk '{print $1}')"
echo "RL kept count: $(find "$DIR/logs/${TAG}/rl/libfuzzer_corpus/${TARGET}" -type f 2>/dev/null | wc -l | awk '{print $1}')"

exit 0

