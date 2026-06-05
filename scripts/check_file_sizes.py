#!/usr/bin/env python3
"""T2 R1: no first-party Rust file exceeds 800 lines, not counting #[cfg(test)] items.

With no argument it checks every first-party file except test files. With `--family NAME` it
checks only that family's files, test-looking names included, because nothing created from the
five oversized files may hide behind a name.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from moves_check import FAMILIES, REPO, family_paths  # noqa: E402
from rust_scan import (  # noqa: E402
    _CFG_TEST,
    _item_end,
    first_party_rs_files,
    is_test_file,
    line_of,
    mask,
    rel,
)

LIMIT = 800

# T3 removes both of these exceptions when it splits the two files.
T3_EXCEPTIONS = {
    "nexus-player-frontend/src-tauri/src/commands.rs",
    "nexus-player-frontend/src-tauri/src/dto.rs",
}


def total_lines(text):
    """Newline characters, plus one only if text follows the last newline."""
    count = text.count("\n")
    return count + 1 if text and not text.endswith("\n") else count


def cfg_test_lines(text):
    """The set of lines from each #[cfg(test)] through the closing of the item it marks."""
    code = mask(text)[0]
    lines = set()
    position = 0
    while True:
        match = _CFG_TEST.search(code, position)
        if not match:
            return lines
        end = len(code) if match.group(1) else _item_end(code, match.end())
        last_line = line_of(code, max(match.start(), end - 1))
        lines.update(range(line_of(code, match.start()), last_line + 1))
        position = max(end, match.end())


def counted_lines(text):
    total = total_lines(text)
    return total - len({line for line in cfg_test_lines(text) if line <= total})


def offenders(sources, limit=LIMIT):
    """Return (path, count) for each {path: text} entry above the limit."""
    found = []
    for path, text in sorted(sources.items()):
        count = counted_lines(text)
        if count > limit:
            found.append((path, count))
    return found


def _read(path):
    return (REPO / path).read_bytes().decode("utf-8", errors="replace")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--family", choices=sorted(FAMILIES))
    args = parser.parse_args(argv)

    issues = []
    if args.family:
        paths = family_paths(args.family)
        if not paths:
            issues.append(f"family {args.family} has no files")
    else:
        files, issues = first_party_rs_files()
        paths = [
            path
            for path in map(rel, files)
            if not is_test_file(path) and path not in T3_EXCEPTIONS
        ]

    for issue in issues:
        print(f"FAIL [R1]: {issue}", file=sys.stderr)
    found = offenders({path: _read(path) for path in paths})
    for path, count in found:
        print(f"FAIL [R1]: {path} has {count} lines, the limit is {LIMIT}", file=sys.stderr)
    if issues or found:
        return 1
    print(f"ok [R1]: {len(paths)} files are within {LIMIT} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
