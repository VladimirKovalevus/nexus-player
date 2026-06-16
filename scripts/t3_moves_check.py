#!/usr/bin/env python3
"""T3 R2, R3, R5, R8, R13, R16: the src-tauri splits are moves only, and the tree is guarded.

- Families `commands` and `dto` (the original file plus every .rs file under the directory named
  after it) go through moves_check.compare_sources unchanged, so T2's rules apply exactly.
- events.rs is a one-file family: only its module-level `use` lines may change.
- lib.rs is compared with its base as a token stream. Allowed: handler entries in
  `generate_handler![...]` that name a child module path with the same last segments in the
  same order, and one module-level `#[cfg(test)] mod wire_format_tests;`.
- The tree guard lists every changed or new path against the base and accepts only what T3 may
  touch, with content rules for the licence files and the manifests.

Standard library only. The functions take plain strings and dicts, so the self-tests need no
git; `main` reads the base from git and the current tree from disk.
"""

import difflib
import json
import os
import re
import subprocess
import sys
import tomllib
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(__file__).resolve().parent))

from moves_check import compare_sources, tokenize  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
BASE_FILE = "scripts/acceptance/T3_BASE_SHA"
SRC = "nexus-player-frontend/src-tauri/src"
FAMILIES = {
    "commands": f"{SRC}/commands.rs",
    "dto": f"{SRC}/dto.rs",
}
LIB = f"{SRC}/lib.rs"
EVENTS = f"{SRC}/events.rs"
WIRE_TESTS = f"{SRC}/wire_format_tests.rs"
DECLARATION = ("#", "[", "cfg", "(", "test", ")", "]", "mod", "wire_format_tests", ";")

SCRIPTS_CHANGEABLE = {"scripts/check_file_sizes.py", "scripts/tests/test_check_file_sizes.py"}
DOCS = {"README.md", "THIRD_PARTY_NOTICES.md", "crates/streaming/README.md"}
LICENSES = {"LICENSE", "crates/streaming/LICENSE"}
DELETABLE = {"types.md", "CONTRIBUTING.md"}
MANIFESTS = {
    "nexus-player-backend/Cargo.toml": ("license", "authors", "repository"),
    "nexus-player-frontend/src-tauri/Cargo.toml": (
        "license",
        "authors",
        "repository",
        "description",
    ),
    "crates/streaming/Cargo.toml": ("license", "authors", "repository"),
}
PACKAGE_JSON = "nexus-player-frontend/package.json"
CI = ".github/workflows/ci.yml"
RAW_MD = "vendor/butterchurn/raw.md"

VAGUE_STEM = re.compile(
    r"(part[0-9]*|misc|util|utils|helper|helpers|common|other|extra|rest|impl[0-9]*)"
)
SHADOWING = {"streaming", "tauri", "std", "serde", "nexus_player_backend"}


# --- families --------------------------------------------------------------------------------


def family_of(path):
    """The family name of a family file path, or None."""
    for name, root in FAMILIES.items():
        prefix = root[: -len(".rs")] + "/"
        if path == root or (path.startswith(prefix) and path.endswith(".rs")):
            return name
    return None


def vague_name_findings(paths):
    """R3: new family files are named for their responsibility and shadow no crate."""
    findings = []
    for path in sorted(paths):
        name = family_of(path)
        if name is None or path == FAMILIES[name]:
            continue
        pure = PurePosixPath(path)
        stem = pure.parent.name if pure.name == "mod.rs" else pure.stem
        if VAGUE_STEM.fullmatch(stem):
            findings.append(f"[{name}] {path}: `{stem}` is not named for its responsibility")
        if stem in SHADOWING:
            findings.append(f"[{name}] {path}: `{stem}` would shadow the crate of that name")
    return findings


# --- lib.rs ----------------------------------------------------------------------------------


def _texts(tokens):
    return [token.text for token in tokens]


def _remove_declaration(tokens):
    """Return (tokens without one module-level declaration, findings)."""
    texts = _texts(tokens)
    width = len(DECLARATION)
    depth = 0
    found = []
    for index, token in enumerate(tokens):
        if token.kind == "punct" and token.text == "{":
            depth += 1
        elif token.kind == "punct" and token.text == "}":
            depth -= 1
        elif depth == 0 and tuple(texts[index : index + width]) == DECLARATION:
            found.append(index)
    findings = []
    if len(found) > 1:
        findings.append(
            f"[lib] {LIB}:{tokens[found[1]].line}: `#[cfg(test)] mod wire_format_tests;` "
            f"occurs {len(found)} times, once is allowed"
        )
    if found:
        first = found[0]
        tokens = tokens[:first] + tokens[first + width :]
    return tokens, findings


def _handler_group(tokens):
    """(index of `[`, index of the matching `]`) of `generate_handler ! [`, or None."""
    texts = _texts(tokens)
    for index in range(len(tokens) - 2):
        if texts[index : index + 3] == ["generate_handler", "!", "["]:
            depth = 0
            for close in range(index + 2, len(tokens)):
                if tokens[close].kind != "punct":
                    continue
                if tokens[close].text in "([{":
                    depth += 1
                elif tokens[close].text in ")]}":
                    depth -= 1
                    if depth == 0:
                        return index + 2, close
            return None
    return None


def _split_entries(tokens):
    entries, current = [], []
    for token in tokens:
        if token.kind == "punct" and token.text == ",":
            entries.append(current)
            current = []
        else:
            current.append(token)
    if current:
        entries.append(current)
    return entries


def _entry_segments(entry):
    """The path segments of `commands :: a :: b`, or None when the entry has another form."""
    texts = _texts(entry)
    kinds = [token.kind for token in entry]
    if not texts or len(texts) % 3 != 1:
        return None
    segments = []
    for position in range(0, len(texts), 3):
        if kinds[position] != "ident":
            return None
        segments.append(texts[position])
        if position + 1 < len(texts) and texts[position + 1 : position + 3] != [":", ":"]:
            return None
    return segments


def handler_entries(tokens, open_index, close_index):
    """Each entry of the handler group as (segments or None, first token)."""
    result = []
    for entry in _split_entries(tokens[open_index + 1 : close_index]):
        if entry:
            result.append((_entry_segments(entry), entry[0]))
    return result


def _diff_findings(base_tokens, current_tokens, where):
    findings = []
    matcher = difflib.SequenceMatcher(
        a=_texts(base_tokens), b=_texts(current_tokens), autojunk=False
    )
    for tag, a1, a2, b1, b2 in matcher.get_opcodes():
        if tag == "equal":
            continue
        if b1 < len(current_tokens):
            line = current_tokens[b1].line
        else:
            line = current_tokens[-1].line if current_tokens else 0
        found = " ".join(_texts(current_tokens[b1:b2])[:8]) or "nothing"
        expected = " ".join(_texts(base_tokens[a1:a2])[:8]) or "nothing"
        findings.append(
            f"[lib] {LIB}:{line}: {where}: found {found!r} where the base has {expected!r}"
        )
    return findings


def lib_findings(base_text, current_text):
    """R3: lib.rs differs from its base only by handler path prefixes and the declaration."""
    base_tokens = tokenize(base_text)
    current_tokens, findings = _remove_declaration(tokenize(current_text))
    base_group = _handler_group(base_tokens)
    current_group = _handler_group(current_tokens)
    if base_group is None:
        return [f"[lib] {LIB}: the base has no generate_handler![...]"]
    if current_group is None:
        return findings + [f"[lib] {LIB}: generate_handler![...] is gone"]

    base_open, base_close = base_group
    open_index, close_index = current_group
    findings += _diff_findings(
        base_tokens[: base_open + 1], current_tokens[: open_index + 1], "outside the handler"
    )
    findings += _diff_findings(
        base_tokens[base_close:], current_tokens[close_index:], "outside the handler"
    )

    base_names = []
    for segments, token in handler_entries(base_tokens, base_open, base_close):
        base_names.append(segments[-1] if segments else token.text)
    current_names = []
    for segments, token in handler_entries(current_tokens, open_index, close_index):
        if segments is None or segments[0] != "commands" or len(segments) < 2:
            findings.append(
                f"[lib] {LIB}:{token.line}: handler entry starting with {token.text!r} is not "
                f"of the form commands::...::name"
            )
            current_names.append(None)
        else:
            current_names.append(segments[-1])
    if current_names != base_names:
        matcher = difflib.SequenceMatcher(a=base_names, b=current_names, autojunk=False)
        for tag, a1, a2, b1, b2 in matcher.get_opcodes():
            if tag != "equal":
                findings.append(
                    f"[lib] {LIB}: handler names differ from the base in order or content: "
                    f"found {current_names[b1:b2][:6]}, the base has {base_names[a1:a2][:6]}"
                )
    return findings


# --- tree guard ------------------------------------------------------------------------------


def _license_findings(path, base, current):
    base_lines = base.split("\n")
    current_lines = current.split("\n")
    copyright_lines = [i for i, line in enumerate(current_lines) if line.startswith("Copyright")]
    base_copyright = [i for i, line in enumerate(base_lines) if line.startswith("Copyright")]
    if len(copyright_lines) != 1 or len(base_copyright) != 1:
        return [f"[tree] {path}: expected exactly one line starting with Copyright"]
    if len(base_lines) != len(current_lines) or copyright_lines != base_copyright:
        return [f"[tree] {path}: differs from the base beyond its Copyright line"]
    for index, (old, new) in enumerate(zip(base_lines, current_lines)):
        if index != base_copyright[0] and old != new:
            return [f"[tree] {path}:{index + 1}: differs from the base beyond its Copyright line"]
    return []


def _manifest_findings(path, base, current):
    try:
        base_data = tomllib.loads(base)
        current_data = tomllib.loads(current)
    except tomllib.TOMLDecodeError as error:
        return [f"[tree] {path}: does not parse as TOML: {error}"]
    for data in (base_data, current_data):
        package = data.get("package")
        if isinstance(package, dict):
            for key in MANIFESTS[path]:
                package.pop(key, None)
    if base_data != current_data:
        return [f"[tree] {path}: differs from the base beyond the allowed package keys"]
    return []


def _package_json_findings(base, current):
    try:
        base_data = json.loads(base)
        current_data = json.loads(current)
    except json.JSONDecodeError as error:
        return [f"[tree] {PACKAGE_JSON}: does not parse as JSON: {error}"]
    if not isinstance(current_data, dict):
        return [f"[tree] {PACKAGE_JSON}: is not a JSON object"]
    base_data.pop("license", None)
    current_data.pop("license", None)
    if base_data != current_data:
        return [f"[tree] {PACKAGE_JSON}: differs from the base beyond the license key"]
    return []


def guard_findings(paths, locked_scripts, base_text, current_text):
    """R16: findings for changed or new paths outside what T3 may touch.

    locked_scripts is the set of scripts/ paths at the base that must stay unchanged.
    base_text(path) and current_text(path) return the text, or None when the file is absent.
    """
    findings = []
    for path in sorted(set(paths)):
        if family_of(path) or path in (LIB, EVENTS, WIRE_TESTS, CI, RAW_MD) or path in DOCS:
            continue
        if path.startswith("scripts/"):
            if path in locked_scripts and path not in SCRIPTS_CHANGEABLE:
                findings.append(f"[tree] {path}: T1 and T2 tooling is locked")
            continue
        current = current_text(path)
        base = base_text(path)
        if path in LICENSES:
            if current is None or base is None:
                findings.append(f"[tree] {path}: must exist, with only its Copyright line changed")
            else:
                findings += _license_findings(path, base, current)
        elif path in DELETABLE:
            if current is not None or base is None:
                findings.append(f"[tree] {path}: may only be deleted")
        elif path in MANIFESTS:
            if current is None or base is None:
                findings.append(f"[tree] {path}: must exist")
            else:
                findings += _manifest_findings(path, base, current)
        elif path == PACKAGE_JSON:
            if current is None or base is None:
                findings.append(f"[tree] {path}: must exist")
            else:
                findings += _package_json_findings(base, current)
        else:
            findings.append(f"[tree] {path}: T3 may not touch this path")
    return findings


# --- git and disk ----------------------------------------------------------------------------


def _git(*args, check=True):
    done = subprocess.run(["git", *args], cwd=REPO, capture_output=True)
    if check and done.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {done.stderr.decode(errors='replace')}")
    return done


def base_sha_problems(text, git=_git):
    """Why the content of T3_BASE_SHA cannot serve as the base, or an empty list."""
    sha = text.strip()
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        return [f"[base] {BASE_FILE} does not hold a 40-hex commit: {sha!r}"]
    if git("cat-file", "-e", f"{sha}^{{commit}}", check=False).returncode != 0:
        return [f"[base] {sha} is not a commit in this repository"]
    if git("merge-base", "--is-ancestor", sha, "HEAD", check=False).returncode != 0:
        return [f"[base] {sha} is neither HEAD nor an ancestor of it"]
    if git("cat-file", "-e", f"{sha}:{BASE_FILE}", check=False).returncode == 0:
        return [f"[base] {sha} already contains {BASE_FILE}, so it is not the start of the run"]
    return []


def read_base_sha():
    """The T3 base commit; raises SystemExit with a FAIL line when it is unusable."""
    base_file = REPO / BASE_FILE
    if not base_file.is_file():
        print(f"FAIL [R16]: {BASE_FILE} does not exist", file=sys.stderr)
        raise SystemExit(1)
    text = base_file.read_text(encoding="utf-8")
    problems = base_sha_problems(text)
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        print("FAIL [R16]: the T3 base is unusable", file=sys.stderr)
        raise SystemExit(1)
    return text.strip()


def _nul_separated(data):
    return [item for item in data.decode(errors="surrogateescape").split("\0") if item]


def changed_paths(sha):
    """Changed paths against the base plus non-empty untracked files outside .claude/."""
    changed = _nul_separated(_git("diff", "--name-only", "--no-renames", "-z", sha).stdout)
    untracked = _nul_separated(_git("ls-files", "--others", "--exclude-standard", "-z").stdout)
    substantial = [
        path
        for path in untracked
        if not path.startswith(".claude/") and os.lstat(REPO / path).st_size != 0
    ]
    return changed + substantial


def base_source(sha, path):
    done = _git("show", f"{sha}:{path}", check=False)
    if done.returncode != 0:
        return None
    return done.stdout.decode("utf-8", errors="replace")


def current_source(path):
    full = REPO / path
    if not full.is_file():
        return None
    return full.read_bytes().decode("utf-8", errors="replace")


def family_sources(name):
    """Every file of a family that exists now, as {repository-relative path: text}."""
    root = FAMILIES[name]
    paths = []
    if (REPO / root).is_file():
        paths.append(root)
    directory = REPO / root[: -len(".rs")]
    if directory.is_dir():
        paths += sorted(path.relative_to(REPO).as_posix() for path in directory.rglob("*.rs"))
    return {path: current_source(path) for path in paths}


def locked_scripts(sha):
    listed = _git("ls-tree", "-r", "-z", "--name-only", sha, "--", "scripts").stdout
    return set(_nul_separated(listed))


def all_findings(sha):
    findings = []
    for name, root in FAMILIES.items():
        current = family_sources(name)
        findings += compare_sources({root: base_source(sha, root)}, current, name)
        findings += vague_name_findings(current)
    events_now = current_source(EVENTS)
    if events_now is None:
        findings.append(f"[events] {EVENTS}: the file no longer exists")
    else:
        findings += compare_sources(
            {EVENTS: base_source(sha, EVENTS)}, {EVENTS: events_now}, "events"
        )
    lib_now = current_source(LIB)
    if lib_now is None:
        findings.append(f"[lib] {LIB}: the file no longer exists")
    else:
        findings += lib_findings(base_source(sha, LIB), lib_now)
    findings += guard_findings(
        changed_paths(sha),
        locked_scripts(sha),
        lambda path: base_source(sha, path),
        current_source,
    )
    return findings


def declaration_findings(text):
    """R4: lib.rs declares the wire-format tests once, at module level, under #[cfg(test)]."""
    tokens = tokenize(text or "")
    remaining, findings = _remove_declaration(tokens)
    if len(remaining) == len(tokens):
        findings.append(f"FAIL [R4]: {LIB} does not declare #[cfg(test)] mod wire_format_tests;")
    return findings


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else argv
    if arguments == ["--wire-declared"]:
        findings = declaration_findings(current_source(LIB))
        for finding in findings:
            print(finding, file=sys.stderr)
        if findings:
            return 1
        print(f"ok [R4]: {LIB} declares the wire-format tests")
        return 0
    if arguments:
        print("FAIL [usage]: expected no argument or --wire-declared", file=sys.stderr)
        return 2
    sha = read_base_sha()
    findings = all_findings(sha)
    for finding in findings[:60]:
        print(finding, file=sys.stderr)
    if len(findings) > 60:
        print(f"... and {len(findings) - 60} more findings", file=sys.stderr)
    if findings:
        print(f"FAIL [R2 R3 R5 R16]: {len(findings)} findings against {sha}", file=sys.stderr)
        return 1
    print(f"ok [R2 R3 R5 R16]: commands, dto, events.rs and lib.rs are moves of {sha}, "
          "and the tree guard holds")
    return 0


if __name__ == "__main__":
    sys.exit(main())
