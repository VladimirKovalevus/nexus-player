#!/usr/bin/env python3
"""T3 R3: the Tauri command set is unchanged and every handler names the file defining it.

Names are extracted mechanically, never from a hand-written list:
- function names: on masked code, each `#[tauri::command]` followed, after further attributes
  and doc comments, by `pub fn NAME` or `pub async fn NAME`;
- handler names: the last path segment of each entry of `generate_handler![...]` in lib.rs;
- invoke names: the literal first argument of `invoke(...)` or `invoke<...>(...)` calls in
  nexus-player-frontend/src/**/*.ts and *.vue.

`python3 scripts/t3_commands_check.py` asserts the split state (split-commands and final).
`--base-only` asserts only the base facts and the invoke rule, which hold on the unmodified tree.
"""

import re
import sys
from collections import Counter
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check_file_sizes import line_of, mask  # noqa: E402
from moves_check import tokenize  # noqa: E402
import t3_moves_check as moves  # noqa: E402

COMMANDS = moves.FAMILIES["commands"]
COMMANDS_DIR = COMMANDS[: -len(".rs")]
BASE_COUNT = 88
FRONTEND = "nexus-player-frontend/src"

_COMMAND_ATTR = re.compile(r"#\s*\[\s*tauri\s*::\s*command\s*\]")
_PUB_FN = re.compile(r"pub\s+(?:async\s+)?fn\s+([A-Za-z_]\w*)")
_INVOKE = re.compile(r"""\binvoke\s*(?:<[^()]*?>)?\s*\(\s*(['"`])([a-z0-9_]+)\1""")


def _skip_attributes(code, position):
    while True:
        while position < len(code) and code[position].isspace():
            position += 1
        if not code.startswith("#", position):
            return position
        bracket = position + 1
        while bracket < len(code) and code[bracket].isspace():
            bracket += 1
        if bracket >= len(code) or code[bracket] != "[":
            return position
        depth = 0
        for index in range(bracket, len(code)):
            if code[index] == "[":
                depth += 1
            elif code[index] == "]":
                depth -= 1
                if depth == 0:
                    position = index + 1
                    break
        else:
            return len(code)


def command_fns(path, text):
    """Return ([(name, line)], findings) for one file."""
    code = mask(text)
    found, findings = [], []
    for match in _COMMAND_ATTR.finditer(code):
        start = _skip_attributes(code, match.end())
        fn = _PUB_FN.match(code, start)
        if fn:
            found.append((fn.group(1), line_of(code, fn.start())))
        else:
            findings.append(
                f"FAIL [R3]: {path}:{line_of(code, match.start())}: #[tauri::command] is not "
                f"followed by `pub fn` or `pub async fn`"
            )
    return found, findings


def handler_paths(lib_text):
    """The segment lists of the generate_handler! entries, or None without the macro."""
    tokens = tokenize(lib_text)
    group = moves._handler_group(tokens)
    if group is None:
        return None
    entries = []
    for segments, token in moves.handler_entries(tokens, *group):
        entries.append(segments if segments else [token.text])
    return entries


def invoke_literals(texts):
    """The set of literal invoke names in {path: text}."""
    names = set()
    for text in texts.values():
        names.update(match.group(2) for match in _INVOKE.finditer(text))
    return names


def _module_file(sources, modules):
    """The family file a `commands::m1::...` path resolves to under rustc's rules."""
    directory = PurePosixPath(COMMANDS_DIR).joinpath(*modules[:-1])
    candidates = [
        (directory / f"{modules[-1]}.rs").as_posix(),
        (directory / modules[-1] / "mod.rs").as_posix(),
    ]
    found = [candidate for candidate in candidates if candidate in sources]
    return found[0] if len(found) == 1 else None, candidates


def base_findings(base_commands, base_lib):
    """Return (base name multiset, findings) for the base facts."""
    found, findings = command_fns(COMMANDS, base_commands)
    names = Counter(name for name, _ in found)
    if len(names) != BASE_COUNT:
        findings.append(
            f"FAIL [R3]: the base commands.rs has {len(names)} command names, expected {BASE_COUNT}"
        )
    entries = handler_paths(base_lib)
    if entries is None:
        findings.append("FAIL [R3]: the base lib.rs has no generate_handler![...]")
    elif {entry[-1] for entry in entries} != set(names):
        findings.append("FAIL [R3]: the base handler set differs from the base command set")
    return names, findings


def invoke_findings(base_names, frontend_now, frontend_base):
    """Every literal invoke name is a command; if some were not at the base, none is added."""
    known = set(base_names)
    unmatched_now = invoke_literals(frontend_now) - known
    unmatched_base = invoke_literals(frontend_base) - known
    added = sorted(unmatched_now - unmatched_base)
    if added:
        return [f"FAIL [R3]: frontend invoke names outside the command set: {', '.join(added)}"]
    return []


def split_findings(base_names, sources, lib_text):
    """R3 for the split tree. sources is {path: text} of the commands family."""
    findings = []
    defined = {}
    for path, text in sorted(sources.items()):
        found, problems = command_fns(path, text)
        findings += problems
        for name, line in found:
            defined.setdefault(name, []).append((path, line))
            if path == COMMANDS:
                findings.append(
                    f"FAIL [R3]: {path}:{line}: #[tauri::command] {name} is still in commands.rs"
                )
    counts = Counter({name: len(places) for name, places in defined.items()})
    if set(counts) != set(base_names):
        missing = sorted(set(base_names) - set(counts))
        extra = sorted(set(counts) - set(base_names))
        findings.append(
            f"FAIL [R3]: #[tauri::command] names differ from the base: missing {missing}, "
            f"new {extra}"
        )
    elif counts != base_names:
        findings.append("FAIL [R3]: the number of definitions per command name changed")
    for name, places in sorted(defined.items()):
        files = {path for path, _ in places}
        if len(files) > 1:
            findings.append(
                f"FAIL [R3]: the variants of {name} are split over {', '.join(sorted(files))}"
            )

    entries = handler_paths(lib_text)
    if entries is None:
        return findings + ["FAIL [R3]: lib.rs has no generate_handler![...]"]
    if len(entries) != BASE_COUNT:
        findings.append(f"FAIL [R3]: generate_handler! has {len(entries)} entries, expected 88")
    if {entry[-1] for entry in entries} != set(base_names):
        findings.append("FAIL [R3]: the handler names differ from the base command set")
    for entry in entries:
        shown = "::".join(entry)
        if len(entry) < 3 or entry[0] != "commands":
            findings.append(
                f"FAIL [R3]: handler entry {shown} does not name a child module of commands"
            )
            continue
        path, candidates = _module_file(sources, entry[1:-1])
        if path is None:
            findings.append(
                f"FAIL [R3]: handler entry {shown} resolves to none or both of "
                f"{' and '.join(candidates)}"
            )
        elif entry[-1] not in {name for name, _ in command_fns(path, sources[path])[0]}:
            findings.append(
                f"FAIL [R3]: handler entry {shown}: {path} defines no #[tauri::command] "
                f"{entry[-1]}"
            )
    return findings


def _frontend_now():
    root = moves.REPO / FRONTEND
    return {
        path.relative_to(moves.REPO).as_posix(): path.read_text(encoding="utf-8", errors="replace")
        for path in sorted(root.rglob("*"))
        if path.suffix in (".ts", ".vue") and path.is_file()
    }


def _frontend_base(sha):
    listed = moves._git("ls-tree", "-r", "-z", "--name-only", sha, "--", FRONTEND).stdout
    return {
        path: moves.base_source(sha, path)
        for path in moves._nul_separated(listed)
        if path.endswith((".ts", ".vue"))
    }


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else argv
    if arguments not in ([], ["--base-only"]):
        print("FAIL [usage]: expected no argument or --base-only", file=sys.stderr)
        return 2
    sha = moves.read_base_sha()
    names, findings = base_findings(
        moves.base_source(sha, COMMANDS), moves.base_source(sha, moves.LIB)
    )
    frontend_base = _frontend_base(sha)
    findings += invoke_findings(names, _frontend_now(), frontend_base)
    if not arguments:
        lib_text = moves.current_source(moves.LIB) or ""
        findings += split_findings(names, moves.family_sources("commands"), lib_text)
    for finding in findings:
        print(finding, file=sys.stderr)
    if findings:
        return 1
    unmatched = sorted(invoke_literals(frontend_base) - set(names))
    note = f"; invoke names outside the set already at the base: {unmatched}" if unmatched else ""
    print(f"ok [R3]: {len(names)} command names hold{note}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
