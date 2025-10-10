# Alma: RL Fuzzing Scheduler for Ethereum Consensus Specifications

## How to Run (Demo Playbook)

This is a minimal, reproducible set of steps to run a baseline vs RL comparison for Lighthouse using Honggfuzz, and inspect the outputs you care about.

### Prerequisites

- OS: Linux or macOS (x86_64 or Apple Silicon). On macOS, use Docker Desktop or Colima.
- Docker: Engine 20+ with BuildKit; able to run `docker build` and `docker run`.
  - Resources: 4+ CPU, 8–16 GB RAM, 20+ GB free disk (images + corpora + logs).
- Make + Bash available on host.
- Network: Access to GitHub, Docker registries, and crates.io (prefetch steps will pull crates inside the container).

### Build & Setup

Repository context: https://github.com/RiemaLabs/alma

Build the Lighthouse image and prefetch dependencies (Docker‑based runs):

```bash
cd eth2fuzz
make lighthouse
make prefetch-lighthouse
```

### Quick A/B on lighthouse_attestation (Honggfuzz)

Run a 60s baseline vs RL comparison (2 threads, 10s segments):

```bash
make cov-attestation fuzzer=honggfuzz T=60 RT=60 S=10 n=2 tag=demo
```

Show detailed fields (new_units / mutated_new):

```bash
SHOW_VERBOSE=1 make cov-attestation fuzzer=honggfuzz T=60 RT=60 S=10 n=2 tag=demo
```

### Inspect Artifacts

- Baseline kept samples:
  - `eth2fuzz/workspace/logs/demo/base/libfuzzer_corpus/lighthouse_attestation`
- RL kept samples:
  - `eth2fuzz/workspace/logs/demo/rl/libfuzzer_corpus/lighthouse_attestation`
- Honggfuzz logs (coverage%, new_units):
  - Baseline: `eth2fuzz/workspace/logs/demo_base/rl/hfuzz/logs/lighthouse_attestation.log`
  - RL: `eth2fuzz/workspace/logs/demo/rl/hfuzz/logs/lighthouse_attestation.log`

### Interpret the Summary

- Focus on `mutated_new_cov` (coverage‑contributing samples). This is the most reliable indicator of persistent improvement.
- Equal coverage% is common on mature corpora; RL gains often appear as a few extra kept samples rather than large % jumps.

### Clean Up

Remove all logs:

```bash
make clean-logs
```

Remove logs for a specific tag:

```bash
make clean-tag TAG=demo
```

Remove temp only (merge_tmp, rl_input, etc.):

```bash
make clean-tmp
```

## License

MIT — see [LICENSE](./LICENSE)

## Thanks

This repository builds upon the open‑source beacon‑fuzz/eth2fuzz work led by Sigma Prime (and contributors), and the broader Ethereum community. Thank you for the foundations and prior art.
