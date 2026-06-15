# Render one gungraun summary.json as a markdown table row:
#   | `name` | ΔIr | ΔCycles |
#
# Per gungraun's v6 schema each summary is a BenchmarkSummary with Callgrind
# metrics at profiles[0].summaries.parts[0].metrics_summary.Callgrind, keyed
# by metric name (Ir, EstimatedCycles, …). diff_pct is a stringified percent
# (string-typed to preserve `inf`).
#
# Thresholds (negligible / bold) are read straight from env so the workflow's
# job env vars stay the single source of truth. Cycles thresholds run ~6×
# looser than Ir: Callgrind cycle counts model cache behaviour and shift ±0.2%
# on code layout alone. Bold sits between the noise band and the fail
# soft-limit, and colours the cell via GitHub's KaTeX inline-math (the only
# way to colour a table cell).

# Signed fixed two-decimal string (1 → "+1.00", -1.04 → "-1.04"). jq's
# tostring strips trailing zeros, so work in cents (×100), pad to 3 digits,
# and insert the decimal point.
def fmt2($x):
  (($x * 100) | round) as $c
  | (if $c < 0 then "-" elif $c > 0 then "+" else "" end) as $sg
  | (if $c < 0 then -$c else $c end | tostring) as $abs_s
  | (if ($abs_s | length) < 3 then ("00" + $abs_s)[-3:] else $abs_s end) as $padded
  | $sg + ($padded | sub("(?<f>\\d\\d)$"; ".\(.f)"));
def fmt_pct(s; negligible_pct; bold_pct):
  if s == null then "—"
  elif (s | tostring | test("inf"; "i")) then
    "⚠️ **\(s | tostring)**"
  else
    (try (s | tonumber) catch null) as $n
    | if $n == null then (s | tostring)
      else
        (if $n < 0 then -$n else $n end) as $abs
        | (if $n > 0 then "⚠️" else "✅" end) as $emoji
        | (($n * 100 | round) / 100) as $rounded
        # When |Δ| rounds to 0.00 at two decimals, "+0%" reads as "exactly
        # nothing changed" which diff_pct rarely is — show "≈0%" instead.
        | (if $rounded == 0 then "≈0" else fmt2($rounded) end) as $num
        | if $abs < negligible_pct then "\($num)%"
          else
            (if $abs >= bold_pct then "**\($num)%**" else "\($num)%" end)
            | "\($emoji) \(.)"
          end
      end
  end;
(env.IR_NEGLIGIBLE_PCT     | tonumber) as $ir_neg  |
(env.IR_BOLD_PCT           | tonumber) as $ir_bold |
(env.CYCLES_NEGLIGIBLE_PCT | tonumber) as $cy_neg  |
(env.CYCLES_BOLD_PCT       | tonumber) as $cy_bold |
. as $b
| ($b.profiles[0].summaries.parts[0].metrics_summary.Callgrind // {}) as $ms
# `function_name id` reads cleanly in the table (e.g. `cold clean`); the full
# module_path (file::group::fn) duplicates the comment header.
| (
    ($b.function_name // "")
    + (if (($b.id // "") | tostring) != "" then " " + ($b.id|tostring) else "" end)
  ) as $name
| "| `" + $name + "` | "
  + fmt_pct((($ms.Ir // {}).diffs // {}).diff_pct; $ir_neg; $ir_bold) + " | "
  + fmt_pct((($ms.EstimatedCycles // {}).diffs // {}).diff_pct; $cy_neg; $cy_bold) + " |"
