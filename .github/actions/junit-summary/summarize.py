"""Render a nextest JUnit report as a markdown test-results table."""

import os
import xml.etree.ElementTree as ET

root = ET.parse(os.environ.get('JUNIT', 'target/nextest/ci/junit.xml')).getroot()
tests = failures = errors = skipped = 0
failed = []
for ts in root.iter('testsuite'):
    for tc in ts.iter('testcase'):
        tests += 1
        if tc.find('failure') is not None:
            failures += 1
            failed.append(f"{tc.get('classname')}::{tc.get('name')}")
        elif tc.find('error') is not None:
            errors += 1
            failed.append(f"{tc.get('classname')}::{tc.get('name')}")
        elif tc.find('skipped') is not None:
            skipped += 1

passed = tests - failures - errors - skipped
verdict = '❌' if failures or errors else '✅'
out = ['### Tests', '',
       '| Result | Ran | Passed | Skipped | Failed |',
       '|:---:|---:|---:|---:|---:|',
       f'| {verdict} | {tests} | {passed} | {skipped} | {failures + errors} |']
if failed:
    out += ['', '| Failed test |', '|---|']
    out += [f'| ❌ `{name}` |' for name in failed]
out.append('')

with open(os.environ.get('OUTPUT', 'test-body.md'), 'w') as fh:
    fh.write('\n'.join(out) + '\n')
