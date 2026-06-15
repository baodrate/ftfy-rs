"""Render lcov tracefiles as a markdown coverage summary.

Reads head/base lcov files and a changed-files list from the environment
(see action.yml) and writes the comment body plus a `total-coverage`
step output.
"""

import os


def parse(path):
    """lcov tracefile -> {source path: {lf, lh, fnf, fnh}} from the
    LF/LH/FNF/FNH records (lines/functions found and hit)."""
    files, cur = {}, None
    with open(path) as fh:
        for line in fh:
            line = line.strip()
            if line.startswith('SF:'):
                cur = {'lf': 0, 'lh': 0, 'fnf': 0, 'fnh': 0}
                files[line[3:]] = cur
            elif cur is None:
                continue
            elif line.startswith('LF:'):
                cur['lf'] = int(line[3:])
            elif line.startswith('LH:'):
                cur['lh'] = int(line[3:])
            elif line.startswith('FNF:'):
                cur['fnf'] = int(line[4:])
            elif line.startswith('FNH:'):
                cur['fnh'] = int(line[4:])
            elif line == 'end_of_record':
                cur = None
    return files


def totals(files):
    t = {'lf': 0, 'lh': 0, 'fnf': 0, 'fnh': 0}
    for v in files.values():
        for k in t:
            t[k] += v[k]
    return t


def pct(hit, found):
    return None if found == 0 else 100.0 * hit / found


def fmt_pct(p, hit=None, found=None):
    if p is None:
        return '—'
    s = f'{p:.1f}%'
    return f'{s} ({hit}/{found})' if hit is not None else s


def fmt_delta(new, old):
    """Δ in percentage points: |Δ| < 0.05pp is noise ("≈0"); drops get
    ⚠️ (bold from 1pp), gains get ✅."""
    if new is None or old is None:
        return '—'
    d = new - old
    if abs(d) < 0.05:
        return '≈0'
    s = f'{d:+.2f}pp'
    if d < 0:
        return f'⚠️ **{s}**' if d <= -1.0 else f'⚠️ {s}'
    return f'✅ {s}'


lcov = os.environ.get('LCOV', 'lcov.info')
base_lcov = os.environ.get('BASE_LCOV', '')
changed_path = os.environ.get('CHANGED_FILES', '')
pr = os.environ.get('PR_NUM', '')
base_ref = os.environ.get('BASE_REF', '')
url = os.environ.get('ARTIFACT_URL', '')
output = os.environ.get('OUTPUT', 'coverage-body.md')

head = parse(lcov)
base = parse(base_lcov) if base_lcov and os.path.exists(base_lcov) else None
ht = totals(head)
bt = totals(base) if base is not None else None
changed = []
if changed_path and os.path.exists(changed_path):
    changed = [l.strip() for l in open(changed_path) if l.strip()]

out = []
if pr and bt is not None:
    out += [f'### Coverage — PR-{pr} vs `{base_ref}`', '']
elif pr:
    out += [f'### Coverage — PR-{pr} (no baseline: the `{base_ref}` run failed)', '']
else:
    out += ['### Coverage', '']
out += ['|  | Lines | Functions |', '|---|---:|---:|']
if bt is not None:
    out.append(f"| `{base_ref}` (base) | {fmt_pct(pct(bt['lh'], bt['lf']), bt['lh'], bt['lf'])} | {fmt_pct(pct(bt['fnh'], bt['fnf']), bt['fnh'], bt['fnf'])} |")
out.append(f"| {'PR' if pr else 'total'} | {fmt_pct(pct(ht['lh'], ht['lf']), ht['lh'], ht['lf'])} | {fmt_pct(pct(ht['fnh'], ht['fnf']), ht['fnh'], ht['fnf'])} |")
if bt is not None:
    out.append(f"| **Δ** | {fmt_delta(pct(ht['lh'], ht['lf']), pct(bt['lh'], bt['lf']))} | {fmt_delta(pct(ht['fnh'], ht['fnf']), pct(bt['fnh'], bt['fnf']))} |")
out.append('')

if pr:
    rows = []
    for path in changed:
        h = head.get(path)
        if h is None:
            continue
        b = base.get(path) if base is not None else None
        if b is not None:
            ld = fmt_delta(pct(h['lh'], h['lf']), pct(b['lh'], b['lf']))
            fd = fmt_delta(pct(h['fnh'], h['fnf']), pct(b['fnh'], b['fnf']))
        else:
            ld = fd = 'new' if base is not None else '—'
        rows.append(f"| `{path}` | {fmt_pct(pct(h['lh'], h['lf']))} | {ld} | {fmt_pct(pct(h['fnh'], h['fnf']))} | {fd} |")
    out += ['**Files changed:**', '']
    if rows:
        out += ['| File | Lines | Δ | Functions | Δ |', '|---|---:|---:|---:|---:|']
        out += rows
    else:
        out.append('_No files with coverage data were changed in this PR._')
    out.append('')

if url:
    out += [f'[Download HTML report]({url})', '']

with open(output, 'w') as fh:
    fh.write('\n'.join(out) + '\n')

lines_pct = pct(ht['lh'], ht['lf'])
val = '' if lines_pct is None else f'{lines_pct:.2f}'
gh_out = os.environ.get('GITHUB_OUTPUT')
if gh_out:
    with open(gh_out, 'a') as fh:
        fh.write(f'total-coverage={val}\n')
