#!/usr/bin/env python3
"""T3 R5: the test names equal the T3 baseline plus wire-format tests, in both directions.

Usage: python3 scripts/t3_test_names.py LISTING [--wire-minimum]

LISTING is the output of `cargo test ... --tests -- --list`. Lines ending in `: test` are kept,
the suffix is stripped and the names are compared with scripts/baselines/t3_test_names.txt. Every
extra name must start with `wire_format_tests::`. With --wire-minimum there are at least 13 of
them, and each R4 type prefix has one.
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BASELINE = "scripts/baselines/t3_test_names.txt"
WIRE = "wire_format_tests::"
WIRE_MINIMUM = 13
PREFIXES = (
    "track_dto",
    "playlist_dto",
    "playlist_item_dto",
    "queue_state_dto",
    "queue_entry_dto",
    "audio_state_dto",
    "search_result_dto",
    "track_state_dto",
    "task_info_dto",
    "eq_settings_dto",
    "eq_preset_dto",
    "create_playlist_request",
    "create_playlist_with_tracks_request",
)


def names_from_listing(text):
    suffix = ": test"
    return {line[: -len(suffix)] for line in text.split("\n") if line.endswith(suffix)}


def test_name_findings(baseline, names, wire_minimum):
    findings = []
    missing = sorted(baseline - names)
    if missing:
        findings.append(f"FAIL [R5]: baseline tests are gone or renamed: {missing[:20]}")
    extra = sorted(names - baseline)
    foreign = [name for name in extra if not name.startswith(WIRE)]
    if foreign:
        findings.append(f"FAIL [R5]: new tests outside {WIRE}: {foreign[:20]}")
    wire = [name for name in extra if name.startswith(WIRE)]
    if wire_minimum:
        if len(wire) < WIRE_MINIMUM:
            findings.append(f"FAIL [R5]: {len(wire)} wire-format tests, expected {WIRE_MINIMUM}")
        for prefix in PREFIXES:
            if not any(name.startswith(f"{WIRE}{prefix}_") for name in wire):
                findings.append(f"FAIL [R4]: no test named {WIRE}{prefix}_...")
    return findings


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else argv
    if not arguments or arguments[1:] not in ([], ["--wire-minimum"]):
        print("FAIL [usage]: expected LISTING [--wire-minimum]", file=sys.stderr)
        return 2
    listing = Path(arguments[0]).read_text(encoding="utf-8", errors="replace")
    baseline = set((REPO / BASELINE).read_text(encoding="utf-8").split())
    names = names_from_listing(listing)
    findings = test_name_findings(baseline, names, bool(arguments[1:]))
    for finding in findings:
        print(finding, file=sys.stderr)
    if findings:
        return 1
    print(f"ok [R5]: {len(names)} test names: the baseline plus {len(names - baseline)} "
          "wire-format tests")
    return 0


if __name__ == "__main__":
    sys.exit(main())
