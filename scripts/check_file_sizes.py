#!/usr/bin/env python3
"""No first-party Rust or .vue file exceeds 800 lines (Rust: not counting #[cfg(test)] items).

Usage, from the repository root:

    python3 scripts/check_file_sizes.py            # every first-party file except test files
    python3 scripts/check_file_sizes.py PATH...    # only the .rs and .vue files at or under PATH

A file's lines are its newline characters, plus one if text follows the last newline. Lines
from a #[cfg(test)] (or #[cfg(all(test, ...))]) attribute through the closing brace or
semicolon of the item it marks do not count; every other line does, blank lines and comments
included. With path arguments, test-looking names are checked too, and arguments that do not
exist are skipped.

Standard library only, with no dependency on other scripts, so it runs anywhere.
"""

import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LIMIT = 800
ROOTS = ("nexus-player-backend", "nexus-player-frontend/src-tauri", "crates/streaming")
VUE_ROOT = "nexus-player-frontend/src"
PRUNED = {"nexus-player-frontend/src-tauri/gen"}

_CFG_TEST = re.compile(
    r"#\s*(!?)\s*\[\s*cfg\s*\(\s*(?:test|all\s*\(\s*test\b[^()]*\))\s*\)\s*\]"
)
_RAW_STRING = re.compile(r'[bc]?r(#*)"')
_CHAR = re.compile(r"'(?:\\.[^']*|[^\\'\n])'")


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def mask(text):
    """Text with comments, strings, raw strings and char literals blanked, newlines kept."""
    out = list(text)
    length = len(text)

    def blank(start, end):
        for index in range(start, end):
            if out[index] != "\n":
                out[index] = " "

    i = 0
    while i < length:
        ch = text[i]
        if text.startswith("//", i):
            end = text.find("\n", i)
            end = length if end < 0 else end
            blank(i, end)
            i = end
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < length and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif ch == '"':
            j = i + 1
            while j < length and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            blank(i, min(j + 1, length))
            i = j + 1
        elif ch in "rbc" and not (i and (text[i - 1].isalnum() or text[i - 1] == "_")):
            raw = _RAW_STRING.match(text, i)
            if raw:
                closing = '"' + raw.group(1)
                end = text.find(closing, raw.end())
                end = length if end < 0 else end + len(closing)
                blank(i, end)
                i = end
            else:
                i += 1
        elif ch == "'":
            char = _CHAR.match(text, i)
            if char:
                blank(i, char.end())
                i = char.end()
            else:
                i += 1  # A lifetime.
        else:
            i += 1
    return "".join(out)


def _matching(code, start, opening, closing):
    depth = 0
    for index in range(start, len(code)):
        if code[index] == opening:
            depth += 1
        elif code[index] == closing:
            depth -= 1
            if depth == 0:
                return index
    return len(code) - 1


def _item_end(code, position):
    """The offset just past the item that starts at position, attributes included."""
    while True:
        while position < len(code) and code[position].isspace():
            position += 1
        if code.startswith("#", position):
            bracket = code.find("[", position)
            position = _matching(code, bracket, "[", "]") + 1
        else:
            break
    nesting = 0
    for index in range(position, len(code)):
        char = code[index]
        if char in "([":
            nesting += 1
        elif char in ")]":
            nesting -= 1
        elif nesting == 0 and char == ";":
            return index + 1
        elif nesting == 0 and char == "{":
            return _matching(code, index, "{", "}") + 1
    return len(code)


def total_lines(text):
    """Newline characters, plus one only if text follows the last newline."""
    count = text.count("\n")
    return count + 1 if text and not text.endswith("\n") else count


def cfg_test_lines(text):
    """The set of lines from each #[cfg(test)] through the closing of the item it marks."""
    code = mask(text)
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
        count = total_lines(text) if path.endswith(".vue") else counted_lines(text)
        if count > limit:
            found.append((path, count))
    return found


def _rel(path):
    return Path(path).relative_to(REPO).as_posix()


def source_files_under(directory):
    found = []
    for current, dirs, files in os.walk(directory):
        dirs[:] = [
            d for d in dirs if d != "target" and _rel(Path(current) / d) not in PRUNED
        ]
        found += [_rel(Path(current) / f) for f in files if f.endswith((".rs", ".vue"))]
    return sorted(found)


def is_test_file(relative):
    return relative.endswith("_tests.rs") or relative.startswith("nexus-player-backend/tests/")


def default_paths():
    """Return (paths, issues). A missing or empty root is an issue, never a silent pass."""
    paths, issues = [], []
    for root in ROOTS:
        if not (REPO / root).is_dir():
            issues.append(f"first-party root {root} does not exist")
            continue
        found = source_files_under(REPO / root)
        if not found:
            issues.append(f"first-party root {root} yields no .rs files")
        paths += [path for path in found if not is_test_file(path)]
    if not (REPO / VUE_ROOT).is_dir():
        issues.append(f"first-party root {VUE_ROOT} does not exist")
    else:
        paths += [p for p in source_files_under(REPO / VUE_ROOT) if p.endswith(".vue")]
    return paths, issues


def argument_paths(arguments):
    """Return (paths, issues) for repository-relative files or directories."""
    paths = []
    for argument in arguments:
        target = REPO / argument
        if target.is_file() and target.name.endswith((".rs", ".vue")):
            paths.append(Path(os.path.normpath(argument)).as_posix())
        elif target.is_dir():
            paths += source_files_under(target)
    paths = sorted(set(paths))
    issues = [] if paths else [f"no .rs or .vue file at or under: {' '.join(arguments)}"]
    return paths, issues


def _read(path):
    return (REPO / path).read_bytes().decode("utf-8", errors="replace")


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else argv
    paths, issues = argument_paths(arguments) if arguments else default_paths()
    for issue in issues:
        print(f"FAIL: {issue}", file=sys.stderr)
    found = offenders({path: _read(path) for path in paths})
    for path, count in found:
        print(f"FAIL: {path} has {count} lines, the limit is {LIMIT}", file=sys.stderr)
    if issues or found:
        return 1
    rs = sum(1 for p in paths if p.endswith(".rs"))
    print(f"ok: {rs} .rs and {len(paths) - rs} .vue files are within {LIMIT} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
