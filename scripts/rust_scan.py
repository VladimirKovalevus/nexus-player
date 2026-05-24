"""Shared helpers for the text scanners over first-party Rust.

Standard library only. `mask` blanks comments and string or char literals so that
patterns only ever see code, and `code_without_tests` also blanks #[cfg(test)] items,
found by brace matching.
"""

import os
import re
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ROOTS = ("nexus-player-backend", "nexus-player-frontend/src-tauri", "crates/streaming")
PRUNED = {"nexus-player-frontend/src-tauri/gen"}

_CFG_TEST = re.compile(
    r"#\s*(!?)\s*\[\s*cfg\s*\(\s*(?:test|all\s*\(\s*test\b[^()]*\))\s*\)\s*\]"
)


def rel(path):
    return Path(path).resolve().relative_to(REPO).as_posix()


def rs_files_under(root):
    found = []
    for directory, dirs, files in os.walk(REPO / root):
        dirs[:] = [
            d
            for d in dirs
            if d != "target" and rel(Path(directory) / d) not in PRUNED
        ]
        found += [Path(directory) / f for f in files if f.endswith(".rs")]
    return sorted(found)


def first_party_rs_files(allow_vendor_fallback=False):
    """Return (files, issues). A missing or empty root is an issue, never a silent pass."""
    files, issues = [], []
    for root in ROOTS:
        if root == "crates/streaming" and not (REPO / root / "Cargo.toml").is_file():
            if allow_vendor_fallback:
                root = "vendor/streaming"
            else:
                issues.append("crates/streaming/Cargo.toml does not exist yet")
                continue
        found = rs_files_under(root)
        if not found:
            issues.append(f"first-party root {root} yields no .rs files")
        files += found
    return files, issues


def is_test_file(relative):
    return relative.endswith("_tests.rs") or relative.startswith(
        "nexus-player-backend/tests/"
    )


def read(path):
    return Path(path).read_text(encoding="utf-8", errors="replace")


def line_of(text, offset):
    return text.count("\n", 0, offset) + 1


def mask(text):
    """Return (code, comment_lines): text with comments and literals blanked, newlines kept."""
    out = list(text)
    comment_lines = set()
    length = len(text)

    def blank(start, end, is_comment=False):
        for index in range(start, end):
            if out[index] != "\n":
                out[index] = " "
        if is_comment:
            comment_lines.update(
                range(line_of(text, start), line_of(text, max(start, end - 1)) + 1)
            )

    i = 0
    while i < length:
        ch = text[i]
        if text.startswith("//", i):
            end = text.find("\n", i)
            end = length if end < 0 else end
            blank(i, end, True)
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
            blank(i, j, True)
            i = j
        elif ch == '"':
            j = i + 1
            while j < length and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            blank(i, min(j + 1, length))
            i = j + 1
        elif ch in "rbc" and not (i and (text[i - 1].isalnum() or text[i - 1] == "_")):
            raw = re.compile(r'[bc]?r(#*)"').match(text, i)
            if raw:
                closing = '"' + raw.group(1)
                end = text.find(closing, raw.end())
                end = length if end < 0 else end + len(closing)
                blank(i, end)
                i = end
            else:
                i += 1
        elif ch == "'":
            char = re.compile(r"'(?:\\.[^']*|[^\\'\n])'").match(text, i)
            if char:
                blank(i, char.end())
                i = char.end()
            else:
                i += 1  # A lifetime.
        else:
            i += 1
    return "".join(out), comment_lines


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
    """Return the offset just past the item that starts at position, attributes included."""
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


def strip_cfg_test(code):
    """Blank every #[cfg(test)] item in masked code."""
    position = 0
    while True:
        match = _CFG_TEST.search(code, position)
        if not match:
            return code
        end = len(code) if match.group(1) else _item_end(code, match.end())
        blanked = "".join(c if c == "\n" else " " for c in code[match.start() : end])
        code = code[: match.start()] + blanked + code[end:]
        position = match.start() + len(blanked)


def code_without_tests(text):
    return strip_cfg_test(mask(text)[0])


def iter_attributes(code):
    """Yield (start_line, end_line, is_inner, body) for every attribute in masked code."""
    for match in re.finditer(r"#\s*(!?)\s*\[", code):
        bracket = match.end() - 1
        end = _matching(code, bracket, "[", "]")
        yield (
            line_of(code, match.start()),
            line_of(code, end),
            bool(match.group(1)),
            code[bracket + 1 : end].strip(),
        )
