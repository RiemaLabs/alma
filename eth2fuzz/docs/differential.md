Differential Fuzzing (LibFuzzer): Lighthouse vs Grandine

Overview
- New libFuzzer target: `diff_lighthouse_grandine_attestation`
- Drives Lighthouse attestation verification and, optionally, an external Grandine "oracle" on the same SSZ input, then compares verdicts.
- Runs locally (no Docker). RL scheduling continues to work via the existing CLI when selecting this target.

Oracle Contract (Grandine)
- The fuzz target can call an external command defined by env var `DIFF_ORACLE_CMD`.
- Protocol:
  - Command line: `$DIFF_ORACLE_CMD --type attestation --state <path/to/state.ssz>`
  - Input: attestation SSZ bytes on stdin.
  - Output: write one of `OK`, `ERR`, `1`, `0`, `true`, `false` to stdout.
  - Exit code: 0 on success, non‑zero to skip (ignored by the fuzzer).

Example wrapper (stub)
```
#!/usr/bin/env bash
set -euo pipefail
TYPE=""
STATE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --type) TYPE="$2"; shift 2;;
    --state) STATE="$2"; shift 2;;
    *) echo "unknown arg: $1" >&2; exit 2;;
  esac
done
if [[ "$TYPE" != "attestation" ]]; then echo ERR; exit 0; fi
# Read SSZ from stdin
tmp_att=$(mktemp)
cat > "$tmp_att"
# TODO: invoke Grandine verification here and print OK/ERR.
echo OK
```

Save as `workspace/tools/grandine_oracle_example.sh` and make it executable.

Running the differential fuzzer
```
cd workspace/libfuzzer/fuzz
export ETH2FUZZ_BEACONSTATE="$(pwd)/../../../workspace/corpora/beaconstate"
export DIFF_ORACLE_CMD="$(pwd)/../../../workspace/tools/grandine_oracle_example.sh"
cargo +nightly fuzz run diff_lighthouse_grandine_attestation \
  "$(pwd)/../../../workspace/corpora/attestation" -- -max_total_time=60
```

Run via eth2fuzz CLI (with RL)
- Single target:
  - `cargo run -- target diff_lighthouse_grandine_attestation --fuzzer Libfuzzer --timeout 600`
- Continuous/RL (filters by substring `diff_`):
  - `cargo run -- continuously -q diff_ --fuzzer Libfuzzer --timeout 600`

Notes
- If `DIFF_ORACLE_CMD` is unset or returns a non‑zero exit code, the run continues as pure Lighthouse fuzzing without comparison.
- A mismatch will `panic!` and be recorded as a crash by libFuzzer.
- For performance, implement the oracle using a long‑running server or a lightweight binary if possible.
