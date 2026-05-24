#!/usr/bin/env python3
"""R4: non-test first-party Rust has no .unwrap(), and no more panic sites than the baseline.

The baseline in scripts/baselines/panic_sites.txt was measured once on the unmodified tree,
so unwrap cannot be traded for expect or panic. Unlike clippy, this sees code under any cfg.
Pass --count to print the current panic-site count without judging it.
"""

import re
import sys

from rust_scan import REPO, code_without_tests, first_party_rs_files, is_test_file, line_of, read, rel

UNWRAP = re.compile(r"\.\s*unwrap\s*\(\s*\)")
PANIC_SITE = re.compile(
    r"\.\s*expect\s*\(|\b(?:panic|unreachable|todo|unimplemented)\s*!\s*[(\[{]"
)
BASELINE = REPO / "scripts/baselines/panic_sites.txt"
SHOWN = 25


def main():
    files, issues = first_party_rs_files(allow_vendor_fallback=True)
    unwraps, sites = [], []
    for path in files:
        relative = rel(path)
        if is_test_file(relative):
            continue
        code = code_without_tests(read(path))
        unwraps += [f"{relative}:{line_of(code, m.start())}" for m in UNWRAP.finditer(code)]
        sites += [f"{relative}:{line_of(code, m.start())}" for m in PANIC_SITE.finditer(code)]

    if "--count" in sys.argv:
        print(len(sites))
        return 0

    baseline = int(BASELINE.read_text().strip())
    failed = False
    for issue in issues:
        print(f"FAIL [R4]: {issue}")
        failed = True
    if unwraps:
        failed = True
        print(f"FAIL [R4]: {len(unwraps)} .unwrap() in non-test first-party Rust, for example:")
        print("\n".join(f"  {site}" for site in unwraps[:SHOWN]))
    if len(sites) > baseline:
        failed = True
        print(f"FAIL [R4]: {len(sites)} expect/panic-style sites, baseline is {baseline}:")
        print("\n".join(f"  {site}" for site in sites))
    if not failed:
        print(f"ok [R4]: no .unwrap(), {len(sites)} panic sites (baseline {baseline})")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
