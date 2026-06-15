#!/usr/bin/env bash
# Assemble the gungraun benchmark comment body: a delta table (one row per
# summary.json, rendered by summary.jq) followed by the full output collapsed.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
summaries_dir="${SUMMARIES_DIR:-target/gungraun}"
output="${OUTPUT:-bench-body.md}"

# benchmarks is a workspace member, so gungraun writes summaries under the
# workspace target at the repo root.
mapfile -t summaries < <(find "$summaries_dir" -type f -name summary.json 2>/dev/null | sort)

{
  if [ "${BASELINE:-}" = "success" ]; then
    if [ -n "${PR_NUM:-}" ]; then
      echo "### gungraun benchmark — PR-${PR_NUM} vs \`${BASE_REF}\`"
    else
      echo "### gungraun benchmark — head vs \`${BASE_REF:-base}\`"
    fi
  else
    echo "### gungraun benchmark (PR head only — no baseline)"
  fi
  echo
  if [ "${#summaries[@]}" -gt 0 ]; then
    echo "| Benchmark | ΔIr | ΔCycles |"
    echo "|---|---:|---:|"
    for f in "${summaries[@]}"; do
      jq -r -f "$here/summary.jq" "$f"
    done
  else
    echo "_No \`summary.json\` files found under \`${summaries_dir}/\`._"
  fi
  echo
  if [ -n "${ARTEFACT_URL:-}" ]; then
    echo "Differential flamegraphs and raw summaries: [download artefact](${ARTEFACT_URL})"
    echo
  fi
  echo "<details><summary>Full gungraun output</summary>"
  echo
  echo '```'
  cat "${BENCH_OUTPUT:-bench-output.txt}"
  echo '```'
  echo
  echo "</details>"
} > "$output"
