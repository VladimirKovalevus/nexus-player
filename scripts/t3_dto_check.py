#!/usr/bin/env python3
"""T3 R2, R5 for split-dto: dto.rs keeps its inline local_source_tests module and defines no
struct or enum outside it, so every DTO lives in a child module.

Usage: python3 scripts/t3_dto_check.py
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check_file_sizes import _item_end, line_of, mask  # noqa: E402
import t3_moves_check as moves  # noqa: E402

_TEST_MODULE = re.compile(
    r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+local_source_tests\s*\{"
)
_DEFINITION = re.compile(r"\b(struct|enum)\s+([A-Za-z_]\w*)")


def dto_findings(text):
    if text is None:
        return ["FAIL [R5]: dto.rs does not exist"]
    code = mask(text)
    modules = list(_TEST_MODULE.finditer(code))
    if len(modules) != 1:
        return [
            f"FAIL [R5]: dto.rs must hold exactly one inline #[cfg(test)] mod local_source_tests, "
            f"found {len(modules)}"
        ]
    start = modules[0].start()
    end = _item_end(code, start)
    findings = []
    for match in _DEFINITION.finditer(code):
        if not start <= match.start() < end:
            findings.append(
                f"FAIL [R2]: dto.rs:{line_of(code, match.start())} still defines "
                f"{match.group(1)} {match.group(2)}"
            )
    return findings


def main():
    findings = dto_findings(moves.current_source(moves.FAMILIES["dto"]))
    for finding in findings:
        print(finding, file=sys.stderr)
    if findings:
        return 1
    print("ok [R2 R5]: dto.rs keeps local_source_tests and defines no struct or enum")
    return 0


if __name__ == "__main__":
    sys.exit(main())
