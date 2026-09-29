#!/usr/bin/env bash
# Run a command inside the fork's dev/measurement container.
#   docker/run.sh cargo build --release -p php-cli
#   docker/run.sh bash
# Mounts the parent of the repo at /work so ../php-src resolves as PLAN.md expects.
# PRIV=1 adds the capabilities perf/samply need.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"          # .../rustyphp (holds php-rust/ and php-src/)
image="${RUSTYPHP_IMAGE:-rustyphp-dev:8.5.7}"
extra=()
if [[ "${PRIV:-0}" == 1 ]]; then
  extra+=(--privileged --security-opt seccomp=unconfined)
fi
tty=(); [[ -t 0 && -t 1 ]] && tty=(-it)
# ${arr[@]+...}: macOS ships bash 3.2, where an empty array trips `set -u`.
exec docker run --rm ${tty[@]+"${tty[@]}"} ${extra[@]+"${extra[@]}"} \
  -v "$root":/work \
  -v rustyphp-target:/target \
  -v rustyphp-cargo-registry:/opt/cargo/registry \
  -v rustyphp-scratch:/scratch \
  -w "${WORKDIR_IN:-/work/php-rust/php-rust}" \
  -e PHPR_ALLOW_MISSING_ORACLE \
  "$image" "$@"
