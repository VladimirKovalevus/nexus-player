#!/usr/bin/env python3
"""T2 R2, R3, R5, R8: the five oversized files were split by moves only.

For each family (one original file plus every .rs file under the directory named after it) the
base file and the current files are reduced to a multiset of (owner, tokens) segments and a bag
of module-level comments. Both must be equal, so the file and the order of items and members are
free, while everything inside an item is compared token by token. The command line also guards
the whole tree against edits outside the families.

Standard library only. `compare_sources` needs no git and is what the self-tests call.
"""

import os
import re
import subprocess
import sys
from collections import Counter, namedtuple
from pathlib import Path, PurePosixPath

REPO = Path(__file__).resolve().parent.parent
BASE_FILE = "scripts/acceptance/T2_BASE_SHA"

FAMILIES = {
    "playback": "nexus-player-backend/src/audio/playback.rs",
    "queue": "nexus-player-backend/src/playlist/playback_queue.rs",
    "state": "nexus-player-backend/src/state/state.rs",
    "manager": "crates/streaming/src/state/manager.rs",
    "hls": "crates/streaming/src/buffers/hls.rs",
}
ALLOWED_TESTS = {
    "nexus-player-backend/tests/public_paths.rs",
    "crates/streaming/tests/public_paths.rs",
}

Token = namedtuple("Token", "kind text line")
Segment = namedtuple("Segment", "owner tokens path kind")
ModDecl = namedtuple("ModDecl", "inline name line")
Reexport = namedtuple("Reexport", "inline child line")

_RAW_STRING = re.compile(r'[bc]?r(#*)"')
_RAW_IDENT = re.compile(r"r#[^\W\d]\w*")
_IDENT = re.compile(r"[^\W\d]\w*")
_NUMBER = re.compile(
    r"0[xXoObB][0-9a-fA-F_]+\w*|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?[\d_]+)?\w*"
)
_CHAR = re.compile(r"b?'(?:\\.[^'\n]*|[^\\'\n])'")
_LIFETIME = re.compile(r"'[^\W\d]\w*")
_RESERVED_PATH = {"self", "super", "crate"}
_FN_QUALIFIERS = {"const", "async", "unsafe", "extern", "default"}


def _line(text, offset, cache):
    """Line number of offset, for offsets visited in increasing order."""
    last_offset, last_line = cache
    last_line += text.count("\n", last_offset, offset)
    cache[0], cache[1] = offset, last_line
    return last_line


def tokenize(text):
    tokens = []
    i, length = 0, len(text)
    cache = [0, 1]

    def add(kind, end):
        tokens.append(Token(kind, text[i:end], _line(text, i, cache)))
        return end

    while i < length:
        ch = text[i]
        if ch.isspace():
            i += 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            end = length if end < 0 else end
            tokens.append(Token("comment", text[i:end].rstrip(), _line(text, i, cache)))
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
            tokens.append(Token("comment", text[i:j].rstrip(), _line(text, i, cache)))
            i = j
        elif ch == '"' or (ch in "bc" and text.startswith('"', i + 1)):
            j = i + 1 if ch == '"' else i + 2
            while j < length and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            i = add("string", min(j + 1, length))
        elif _RAW_STRING.match(text, i):
            raw = _RAW_STRING.match(text, i)
            closing = '"' + raw.group(1)
            end = text.find(closing, raw.end())
            i = add("string", length if end < 0 else end + len(closing))
        elif _RAW_IDENT.match(text, i):
            i = add("ident", _RAW_IDENT.match(text, i).end())
        elif _CHAR.match(text, i):
            i = add("char", _CHAR.match(text, i).end())
        elif ch == "'" and _LIFETIME.match(text, i):
            i = add("lifetime", _LIFETIME.match(text, i).end())
        elif _IDENT.match(text, i):
            i = add("ident", _IDENT.match(text, i).end())
        elif ch.isdigit():
            i = add("number", _NUMBER.match(text, i).end())
        else:
            i = add("punct", i + 1)
    return tokens


def _is_plain_comment(token):
    return token.kind == "comment" and not _is_outer_doc(token) and not _is_inner_doc(token)


def _is_outer_doc(token):
    if token.kind != "comment":
        return False
    text = token.text
    if text.startswith("///"):
        return not text.startswith("////")
    return text.startswith("/**") and not text.startswith("/***") and text != "/**/"


def _is_inner_doc(token):
    return token.kind == "comment" and token.text.startswith(("//!", "/*!"))


def _matching(tokens, start, end):
    """Index of the bracket closing the one at start, or end - 1 when unbalanced."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack = []
    for index in range(start, end):
        token = tokens[index]
        if token.kind != "punct":
            continue
        if token.text in pairs:
            stack.append(pairs[token.text])
        elif stack and token.text == stack[-1]:
            stack.pop()
            if not stack:
                return index
    return end - 1


def _normalize(tokens):
    """Remove pub(super), pub(crate), and any comma directly before a closing token."""
    kept = []
    i = 0
    while i < len(tokens):
        window = [token.text for token in tokens[i : i + 4]]
        if window in (["pub", "(", "super", ")"], ["pub", "(", "crate", ")"]):
            i += 4
            continue
        token = tokens[i]
        if (
            token.kind == "punct"
            and token.text in ")]}{>"
            and kept
            and kept[-1].kind == "punct"
            and kept[-1].text == ","
        ):
            kept.pop()
        kept.append(token)
        i += 1
    return kept


def _strip_leading_super(tokens):
    kept = []
    i = 0
    while i < len(tokens):
        window = [token.text for token in tokens[i : i + 3]]
        previous = kept[-1].text if kept else ""
        if window == ["super", ":", ":"] and previous != ":":
            i += 3
            continue
        kept.append(tokens[i])
        i += 1
    return kept


def _texts(tokens):
    return tuple(token.text for token in tokens)


def _reexport_child(tokens):
    """Return the child when tokens are `pub use child::Name;` or `pub use child::{A, B};`."""
    texts = _texts(tokens)
    kinds = [token.kind for token in tokens]
    if len(texts) < 7 or texts[:2] != ("pub", "use") or texts[3:5] != (":", ":"):
        return None
    if kinds[2] != "ident" or texts[2] in _RESERVED_PATH or texts[-1] != ";":
        return None

    def is_name(index):
        return kinds[index] == "ident" and texts[index] not in _RESERVED_PATH

    if len(texts) == 7:
        return texts[2] if is_name(5) else None
    if texts[5] != "{" or texts[-2] != "}":
        return None
    inner = list(range(6, len(texts) - 2))
    if not inner:
        return None
    for position, index in enumerate(inner):
        if position % 2 == 0 and not is_name(index):
            return None
        if position % 2 == 1 and texts[index] != ",":
            return None
    return texts[2]


class ParsedFile:
    def __init__(self, path):
        self.path = path
        self.segments = []
        self.comments = []
        self.mod_decls = []
        self.reexports = []


def _skip_outer_attributes(tokens, start, end):
    """Return (index of the first token after attributes, attribute spans, saw_comment)."""
    i, spans, saw_comment = start, [], False
    while i < end:
        token = tokens[i]
        if token.kind == "comment":
            saw_comment = saw_comment or _is_outer_doc(token)
            i += 1
        elif (
            token.text == "#"
            and i + 1 < end
            and tokens[i + 1].text == "["
            and token.kind == "punct"
        ):
            close = _matching(tokens, i + 1, end)
            spans.append((i, close + 1))
            i = close + 1
        else:
            break
    return i, spans, saw_comment


def _visibility_end(tokens, i, end):
    if i < end and tokens[i].text == "pub":
        if i + 1 < end and tokens[i + 1].text == "(":
            return _matching(tokens, i + 1, end) + 1
        return i + 1
    return i


def _item_kind(tokens, k, end):
    """Classify the item whose first token after visibility sits at k."""
    i = k
    while i < end and (tokens[i].text in _FN_QUALIFIERS or tokens[i].kind == "string"):
        i += 1
    if i < end and tokens[i].text == "fn" and tokens[i].kind == "ident":
        return "fn"
    if i < end and tokens[i].text == "impl":
        if all(tokens[j].text in ("unsafe", "default") for j in range(k, i)):
            return "impl"
    if k < end and tokens[k].text in ("const", "static", "type"):
        return "semicolon"
    return tokens[k].text if k < end else ""


def _generic_end(tokens, k, end, through_braces):
    """Index just past an item: its `;`, or its closing brace unless through_braces."""
    depth = 0
    i = k
    while i < end:
        token = tokens[i]
        if token.kind == "punct":
            if token.text in "([" or (through_braces and token.text == "{"):
                depth += 1
            elif token.text in ")]" or (through_braces and token.text == "}"):
                depth -= 1
            elif depth <= 0 and token.text == ";":
                return i + 1
            elif depth <= 0 and token.text == "{":
                return _matching(tokens, i, end) + 1
        i += 1
    return end


def _parse_items(tokens, start, end, owner, inline, in_impl, parsed):
    i = start
    while i < end:
        token = tokens[i]
        if _is_plain_comment(token):
            parsed.comments.append(token)
            i += 1
            continue
        if _is_inner_doc(token):
            _add_segment(parsed, owner, tokens[i : i + 1], "inner")
            i += 1
            continue
        if token.text == "#" and i + 2 < end and tokens[i + 1].text == "!" and token.kind == "punct":
            close = _matching(tokens, i + 2, end) + 1
            _add_segment(parsed, owner, tokens[i:close], "inner")
            i = close
            continue
        if token.kind == "punct" and token.text == "}":
            _add_segment(parsed, owner, tokens[i : i + 1], "stray")
            i += 1
            continue

        k, spans, saw_doc = _skip_outer_attributes(tokens, i, end)
        vis_start = k
        k = _visibility_end(tokens, k, end)
        visibility = _texts(tokens[vis_start:k])
        kind = _item_kind(tokens, k, end)

        if kind == "impl" and not in_impl:
            i = _parse_impl(tokens, i, k, end, owner, inline, parsed)
        elif kind == "mod" and not in_impl and _is_module(tokens, k, end):
            i = _parse_module(tokens, i, k, end, spans, saw_doc, visibility, owner, inline, parsed)
        elif kind == "use" and not in_impl:
            i = _parse_use(tokens, i, k, end, spans, saw_doc, visibility, owner, inline, parsed)
        else:
            stop = _generic_end(tokens, k, end, kind == "semicolon")
            _add_segment(parsed, owner, tokens[i:stop], "fn" if kind == "fn" else "item")
            i = stop


def _is_module(tokens, k, end):
    return (
        k + 2 < end
        and tokens[k + 1].kind == "ident"
        and tokens[k + 2].kind == "punct"
        and tokens[k + 2].text in ";{"
    )


def _add_segment(parsed, owner, tokens, kind):
    parsed.segments.append(Segment(owner, _normalize(tokens), parsed.path, kind))


def _parse_impl(tokens, i, k, end, owner, inline, parsed):
    brace = k
    depth = 0
    while brace < end:
        token = tokens[brace]
        if token.kind == "punct":
            if token.text in "([":
                depth += 1
            elif token.text in ")]":
                depth -= 1
            elif depth == 0 and token.text == "{":
                break
        brace += 1
    if brace >= end:
        _add_segment(parsed, owner, tokens[i:end], "item")
        return end
    close = _matching(tokens, brace, end)
    header = _normalize(_strip_leading_super(tokens[i : brace + 1]))
    member_owner = owner + _texts(header)
    before = len(parsed.segments)
    _parse_items(tokens, brace + 1, close, member_owner, inline, True, parsed)
    is_trait_impl = any(
        token.text == "for" and tokens[index + 1].text != "<"
        for index, token in enumerate(tokens[k:brace], start=k)
    )
    if is_trait_impl and len(parsed.segments) == before:
        # A trait impl with no members still means something, as with `unsafe impl Send`.
        parsed.segments.append(Segment(member_owner, header, parsed.path, "item"))
    return close + 1


def _parse_module(tokens, i, k, end, spans, saw_doc, visibility, owner, inline, parsed):
    name = tokens[k + 1].text
    if tokens[k + 2].text == ";":
        plain = i == k - len(visibility) and not spans and not saw_doc
        if plain and visibility in ((), ("pub", "(", "super", ")"), ("pub", "(", "crate", ")")):
            parsed.mod_decls.append(ModDecl(inline, name, tokens[k].line))
        else:
            _add_segment(parsed, owner, tokens[i : k + 3], "item")
        return k + 3
    close = _matching(tokens, k + 2, end)
    # The header is a segment of its own, so an inline module absent from the base is a finding
    # even when it holds nothing but a `mod y;` declaration.
    _add_segment(parsed, owner, tokens[i : k + 3], "mod")
    attributes = _normalize(tokens[i : k - len(visibility)])
    inner_owner = owner + _texts(attributes) + ("mod", name)
    _parse_items(tokens, k + 3, close, inner_owner, inline + (name,), False, parsed)
    return close + 1


def _parse_use(tokens, i, k, end, spans, saw_doc, visibility, owner, inline, parsed):
    stop = _generic_end(tokens, k, end, True)
    if visibility == ("pub",):
        plain = i == k - 1 and not spans and not saw_doc
        child = _reexport_child(tokens[k - 1 : stop]) if plain else None
        if child:
            parsed.reexports.append(Reexport(inline, child, tokens[k].line))
        else:
            _add_segment(parsed, owner, tokens[i:stop], "item")
        return stop
    only_cfg = all(_texts(tokens[a + 2 : a + 4]) == ("cfg", "(") for a, _ in spans)
    droppable = visibility in ((), ("pub", "(", "super", ")"), ("pub", "(", "crate", ")"))
    if not (only_cfg and droppable and not saw_doc):
        _add_segment(parsed, owner, tokens[i:stop], "item")
    return stop


def parse_source(path, text):
    tokens = tokenize(text)
    parsed = ParsedFile(path)
    _parse_items(tokens, 0, len(tokens), (), (), False, parsed)
    return parsed


def _module_targets(path, inline, name):
    source = PurePosixPath(path)
    directory = source.parent if source.name == "mod.rs" else source.parent / source.stem
    for part in inline:
        directory = directory / part
    return (directory / f"{name}.rs").as_posix(), (directory / name / "mod.rs").as_posix()


def _reachability(root, parsed_files, family):
    findings = []
    seen = {root}
    pending = [root]
    while pending:
        path = pending.pop()
        for decl in parsed_files[path].mod_decls:
            targets = _module_targets(path, decl.inline, decl.name)
            found = [target for target in targets if target in parsed_files]
            if not found:
                findings.append(
                    f"[{family}] {path}:{decl.line}: `mod {decl.name};` resolves to no family "
                    f"file, expected {targets[0]} or {targets[1]}"
                )
            elif len(found) > 1:
                findings.append(
                    f"[{family}] {path}:{decl.line}: `mod {decl.name};` is ambiguous between "
                    f"{targets[0]} and {targets[1]}"
                )
            for target in found:
                if target not in seen:
                    seen.add(target)
                    pending.append(target)
    for path in sorted(set(parsed_files) - seen):
        findings.append(
            f"[{family}] {path}: no `mod` declares this file, so it is not part of the build"
        )
    return findings


def _segment_key(segment):
    return (segment.owner, _texts(segment.tokens))


def _preview(tokens):
    return " ".join(token.text for token in tokens[:12])


def _common_prefix(left, right):
    count = 0
    for a, b in zip(left, right):
        if a != b:
            break
        count += 1
    return count


def _describe_difference(family, current, base):
    current_texts, base_texts = _texts(current.tokens), _texts(base.tokens)
    index = _common_prefix(current_texts, base_texts)
    if index < len(current.tokens):
        line = current.tokens[index].line
        found = repr(current_texts[index])
    else:
        line = current.tokens[-1].line
        found = "the end of the item"
    if index < len(base.tokens):
        expected = f"{base_texts[index]!r} (base line {base.tokens[index].line})"
    else:
        expected = "the end of the item"
    return (
        f"[{family}] {current.path}:{line}: first differing token is {found}, the base has "
        f"{expected}, in: {_preview(current.tokens)}"
    )


def _surplus(segments, counts):
    """The segments of the list whose key still has a positive count, decrementing it."""
    found = []
    for segment in segments:
        key = _segment_key(segment)
        if counts[key] > 0:
            counts[key] -= 1
            found.append(segment)
    return found


def _compare_segments(base_segments, current_segments, family):
    base_counts = Counter(_segment_key(segment) for segment in base_segments)
    current_counts = Counter(_segment_key(segment) for segment in current_segments)
    missing = _surplus(base_segments, base_counts - current_counts)
    extra = _surplus(current_segments, current_counts - base_counts)

    findings = []
    for segment in extra:
        candidates = [other for other in missing if other.owner == segment.owner]
        if candidates:
            closest = max(
                candidates,
                key=lambda other: _common_prefix(_texts(segment.tokens), _texts(other.tokens)),
            )
            if _common_prefix(_texts(segment.tokens), _texts(closest.tokens)):
                missing.remove(closest)
                findings.append(_describe_difference(family, segment, closest))
                continue
        line = segment.tokens[0].line if segment.tokens else 0
        first = repr(segment.tokens[0].text) if segment.tokens else "nothing"
        findings.append(
            f"[{family}] {segment.path}:{line}: not in the base under owner "
            f"{' '.join(segment.owner) or '(module level)'}, first differing token is {first}, "
            f"in: {_preview(segment.tokens)}"
        )
    for segment in missing:
        line = segment.tokens[0].line if segment.tokens else 0
        first = repr(segment.tokens[0].text) if segment.tokens else "nothing"
        findings.append(
            f"[{family}] {segment.path}:{line}: in the base but missing now, under owner "
            f"{' '.join(segment.owner) or '(module level)'}, first differing token is {first}, "
            f"in: {_preview(segment.tokens)}"
        )
    return findings


def _compare_comments(base_files, current_files, family):
    findings = []
    base_bag = Counter(c.text for parsed in base_files for c in parsed.comments)
    current_bag = Counter(c.text for parsed in current_files for c in parsed.comments)
    for parsed in current_files:
        for comment in parsed.comments:
            if current_bag[comment.text] > base_bag[comment.text]:
                current_bag[comment.text] -= 1
                findings.append(
                    f"[{family}] {parsed.path}:{comment.line}: comment not in the base, "
                    f"first differing token is {comment.text!r}"
                )
    for parsed in base_files:
        for comment in parsed.comments:
            if base_bag[comment.text] > current_bag[comment.text]:
                base_bag[comment.text] -= 1
                findings.append(
                    f"[{family}] {parsed.path}:{comment.line}: base comment is gone, "
                    f"first differing token is {comment.text!r}"
                )
    return findings


def _compare_reexports(parsed_files, family):
    findings = []
    for parsed in parsed_files.values():
        declared = {(decl.inline, decl.name) for decl in parsed.mod_decls}
        for reexport in parsed.reexports:
            if (reexport.inline, reexport.child) not in declared:
                findings.append(
                    f"[{family}] {parsed.path}:{reexport.line}: `pub use {reexport.child}::` "
                    f"needs `mod {reexport.child};` in the same file"
                )
    return findings


def compare_sources(base, current, family="family"):
    """Compare one family. base is {root path: text}, current is {path: text}. Returns findings."""
    if len(base) != 1:
        raise ValueError("base holds exactly the family's original file")
    root = next(iter(base))
    if root not in current:
        return [f"[{family}] {root}: the original file no longer exists"]
    prefix = root[: -len(".rs")] + "/"
    findings = []
    for path in sorted(current):
        if path != root and not (path.startswith(prefix) and path.endswith(".rs")):
            findings.append(f"[{family}] {path}: not a file of this family")

    base_parsed = parse_source(root, base[root])
    current_parsed = {
        path: parse_source(path, text)
        for path, text in sorted(current.items())
        if path == root or (path.startswith(prefix) and path.endswith(".rs"))
    }
    current_segments = [s for parsed in current_parsed.values() for s in parsed.segments]
    findings += _compare_segments(base_parsed.segments, current_segments, family)
    findings += _compare_comments([base_parsed], list(current_parsed.values()), family)
    findings += _compare_reexports(current_parsed, family)
    findings += _reachability(root, current_parsed, family)
    return findings


def family_paths(name):
    """Every file of a family that exists now, as repository-relative posix paths."""
    root = FAMILIES[name]
    paths = []
    if (REPO / root).is_file():
        paths.append(root)
    directory = REPO / root[: -len(".rs")]
    if directory.is_dir():
        paths += sorted(path.relative_to(REPO).as_posix() for path in directory.rglob("*.rs"))
    return paths


def guard_paths(paths):
    """R8, R3, R2: findings for changed or new paths outside what T2 may touch."""
    findings = []
    for path in sorted(paths):
        in_family = any(
            path == root or (path.startswith(root[: -len(".rs")] + "/") and path.endswith(".rs"))
            for root in FAMILIES.values()
        )
        if not (in_family or path.startswith("scripts/") or path in ALLOWED_TESTS):
            findings.append(f"[tree] {path}: T2 may only touch family files, scripts/ and tests")
    return findings


def _git(*args, check=True):
    done = subprocess.run(["git", *args], cwd=REPO, capture_output=True)
    if check and done.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {done.stderr.decode(errors='replace')}")
    return done


def base_sha_problems(text, git=_git):
    """Why the content of T2_BASE_SHA cannot serve as the base, or an empty list."""
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


def _nul_separated(data):
    return [item for item in data.decode(errors="surrogateescape").split("\0") if item]


def tree_findings(sha):
    changed = _nul_separated(_git("diff", "--name-only", "--no-renames", "-z", sha).stdout)
    untracked = _nul_separated(_git("ls-files", "--others", "--exclude-standard", "-z").stdout)
    # The agent sandbox can leave empty placeholders at the root.
    substantial = [path for path in untracked if os.lstat(REPO / path).st_size != 0]
    return guard_paths(changed + substantial)


def base_source(sha, path):
    return _git("show", f"{sha}:{path}").stdout.decode("utf-8", errors="replace")


def current_sources(name):
    return {
        path: (REPO / path).read_bytes().decode("utf-8", errors="replace")
        for path in family_paths(name)
    }


def main():
    base_file = REPO / BASE_FILE
    if not base_file.is_file():
        print(f"FAIL [moves]: {BASE_FILE} does not exist", file=sys.stderr)
        return 1
    findings = base_sha_problems(base_file.read_text(encoding="utf-8"))
    if findings:
        for finding in findings:
            print(finding, file=sys.stderr)
        return 1
    sha = base_file.read_text(encoding="utf-8").strip()
    for name, root in FAMILIES.items():
        findings += compare_sources({root: base_source(sha, root)}, current_sources(name), name)
    findings += tree_findings(sha)
    for finding in findings[:60]:
        print(finding, file=sys.stderr)
    if len(findings) > 60:
        print(f"... and {len(findings) - 60} more findings", file=sys.stderr)
    if findings:
        print(f"FAIL [moves]: {len(findings)} findings against {sha}", file=sys.stderr)
        return 1
    print(f"ok [moves]: all families are moves of {sha}, and the tree guard holds")
    return 0


if __name__ == "__main__":
    sys.exit(main())
