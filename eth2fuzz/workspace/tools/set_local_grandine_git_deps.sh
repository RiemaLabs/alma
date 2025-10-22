#!/usr/bin/env bash
set -euo pipefail

# Rewrite Grandine deps in workspace/libfuzzer/fuzz/Cargo.toml to git=file://<ABS>/grandine

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
GIT_URI="file://${ROOT}/grandine"
TOML="${ROOT}/workspace/libfuzzer/fuzz/Cargo.toml"

if [[ ! -f "$TOML" ]]; then echo "[local] missing $TOML" >&2; exit 1; fi

sed -i '' -E \
  "s#^gtypes = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#gtypes = { git = \"${GIT_URI}\", package = \"types\" }#" \
  "$TOML"
sed -i '' -E \
  "s#^gssz = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*,[[:space:]]*package = \"ssz\"[[:space:]]*\}#gssz = { git = \"${GIT_URI}\", package = \"ssz\" }#" \
  "$TOML"
sed -i '' -E \
  "s#^gtrans = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*\}#gtrans = { git = \"${GIT_URI}\", package = \"transition_functions\" }#" \
  "$TOML"
sed -i '' -E \
  "s#^gpubkey_cache = \{[[:space:]]*path = \"[^\"]*\"[[:space:]]*,[[:space:]]*package = \"pubkey_cache\"[[:space:]]*\}#gpubkey_cache = { git = \"${GIT_URI}\", package = \"pubkey_cache\" }#" \
  "$TOML"

echo "[local] Rewrote Grandine deps to ${GIT_URI}"

