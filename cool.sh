#!/usr/bin/env bash
# Run cargo with a CPU cap so a laptop stays cool.
#   ./cool.sh run            ./cool.sh check            ./cool.sh build --release
#   PCT=35 ./cool.sh run     (default cap: 50% of all threads)
# Uses a systemd CPU quota when available (hard cap), plus nice (low priority).
set -e
PCT=${PCT:-50}
QUOTA=$(( $(nproc) * PCT ))
if command -v systemd-run >/dev/null 2>&1 \
   && systemd-run --user --scope --quiet -p CPUQuota=${QUOTA}% true 2>/dev/null; then
  exec systemd-run --user --scope --quiet -p CPUQuota=${QUOTA}% nice -n 15 cargo "$@"
fi
echo "(no systemd CPU quota available: using nice + 4 jobs only)" >&2
exec nice -n 15 cargo "$@"
