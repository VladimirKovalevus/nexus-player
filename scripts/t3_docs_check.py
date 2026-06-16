#!/usr/bin/env python3
"""T3 R7-R14: documents, licences, metadata and the CI workflow.

Usage: python3 scripts/t3_docs_check.py CHECK, where CHECK is one of
  notices     R12  THIRD_PARTY_NOTICES.md
  raw-md      R13  vendor/butterchurn/raw.md in English, nothing else in vendor/ changed
  license     R8   both LICENSE files name the owner
  metadata    R8   manifests, package.json, cargo metadata, Cargo.lock and tauri.conf.json
  readme      R7 R9 R10  the README, the streaming README, the deleted documents
  disclaimer  R11  the README disclaimer
  ci          R14  .github/workflows/ci.yml

Each check is a pure function over texts and lists, so the self-tests can feed fixtures; the
command line gathers the inputs from git, the tree, package-lock.json and target/t3-ci/.
PyYAML is the only import outside the standard library, and only the ci check needs it.
"""

import json
import posixpath
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from urllib.parse import unquote

sys.path.insert(0, str(Path(__file__).resolve().parent))

import t3_moves_check as moves  # noqa: E402

REPO = moves.REPO
OWNER_LINE = "Copyright (c) 2026 Vladimir Kovalev"
AUTHORS = ["Vladimir Kovalev <vladkoval805@gmail.com>"]
REPOSITORY = "https://github.com/VladimirKovalevus/nexus-player"
CRATES = {
    "nexus-player-backend": "nexus-player-backend/Cargo.toml",
    "nexus-player-frontend": "nexus-player-frontend/src-tauri/Cargo.toml",
    "streaming": "crates/streaming/Cargo.toml",
}
SRC_TAURI_MANIFEST = CRATES["nexus-player-frontend"]
LICENSE_FILES = ("LICENSE", "crates/streaming/LICENSE")
LOCK = "nexus-player-frontend/package-lock.json"
CI_DIR = "target/t3-ci"

# R12: what each vendor section names, from facts section 13 and the LICENSE files at the base.
VENDOR_FACTS = {
    "Symphonia": [
        "github.com/pdeljanov/Symphonia",
        "rel-0.5",
        "bbaf23c",
        "MPL-2.0",
        "symphonia-format-ogg/src/demuxer.rs",
        "symphonia/src/lib.rs",
    ],
    "symphonia-adapters": [
        "github.com/aschey/symphonia-adapters",
        "symphonia-adapter-libopus-v0.2.6",
        "ac436ed",
        "MIT",
        "Apache-2.0",
        "crates/symphonia-adapter-libopus/Cargo.toml",
    ],
    "rsoundcloud": [
        "github.com/barthofu/rsoundcloud",
        "d7196a5",
        "MIT",
        "Bartholomé Gili",
        "src/api/tracks.rs",
        "src/client.rs",
        "src/utils/serde_helpers.rs",
    ],
    "bandcamp": [
        "github.com/fabi321/bandcamp",
        "v0.3.4",
        "bb4864c",
        "MIT",
        "Fabian Wunsch",
        "src/search.rs",
    ],
    "butterchurn": [
        "github.com/jberg/butterchurn",
        "956cf4e",
        "MIT",
        "Jordan Berg",
        "src/audio/audioProcessor.js",
        "src/visualizer.js",
        "src/rendering/renderer.js",
        "raw.md",
    ],
}
# The copyright holders above, as the base LICENSE file of each MIT-only directory states them.
HOLDERS = {
    "rsoundcloud": ("vendor/rsoundcloud/LICENSE", "Bartholomé Gili"),
    "bandcamp": ("vendor/bandcamp/LICENSE", "Fabian Wunsch"),
    "butterchurn": ("vendor/butterchurn/LICENSE", "Jordan Berg"),
}
NPM_PACKAGES = ("butterchurn-presets", "@fontsource/ibm-plex-sans", "@fontsource/ibm-plex-mono")

README_LINKS = (
    "nexus-player-backend/src/playlist/cursor",
    "nexus-player-backend/src/audio/hls_decoder.rs",
    "nexus-player-backend/src/audio/chunked.rs",
    "crates/streaming/src/hls",
    "crates/streaming/src/state/manager.rs",
    "crates/streaming/src/images",
    "nexus-player-backend/src/sync/telegram",
    "vendor/butterchurn",
    "THIRD_PARTY_NOTICES.md",
    "LICENSE",
)
README_NAMES = ("nexus-player-backend", "nexus-player-frontend", "crates/streaming", "vendor")
README_BUILD = (
    "clang",
    "mold",
    "cmake",
    "pkg-config",
    "libwebkit2gtk-4.1-dev",
    "libasound2-dev",
    "npm",
)
FORBIDDEN_README = (
    r"youtube",
    r"\bvk\b",
    r"vk[.]com",
    r"vkontakte",
    r"planned",
    r"coming soon",
    r"roadmap",
    r"\bwip\b",
    r"work in progress",
    r"nexus-monorepo",
    r"syndafloden",
    r"goalflow",
    r"scripts/acceptance",
    r"\bphase [0-9]",
    r"tauri[.]app/v1",
    r"msrv",
    r"rust-version",
    r"artificial intelligence",
    r"\bllm",
    r"\bgpt\b",
    r"claude",
    r"anthropic",
    r"openai",
    r"copilot",
    r"hand-written",
    r"handwritten",
    r"\bby hand\b",
    r"human-written",
    r"bilingual",
    r"multilingual",
    r"i18n",
    r"locali[sz]",
    r"translat",
    r"english",
    r"russian",
)
DISCLAIMER_WORDS = (
    "soundcloud",
    "bandcamp",
    "unofficial",
    "undocumented",
    "not affiliated",
    "distribut",
    "content",
    "educational",
    "personal use",
)

APT_PACKAGES = (
    "clang",
    "mold",
    "cmake",
    "pkg-config",
    "libwebkit2gtk-4.1-dev",
    "libgtk-3-dev",
    "libsoup-3.0-dev",
    "libjavascriptcoregtk-4.1-dev",
    "libasound2-dev",
)
FIRST_PARTY = ("nexus-player-backend", "nexus-player-frontend", "streaming")
CI_FORBIDDEN = (
    "--workspace",
    "vendor/",
    "npm",
    "npx",
    "setup-node",
    "scripts/acceptance",
    "scripts/tests",
    "BASE_SHA",
    "git show",
    "git diff",
    "rev-parse",
    "merge-base",
    "goalflow",
    "secrets.",
    "continue-on-error",
)

_FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
_CLOSING_FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})\s*$")
_HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
_INLINE_CODE = re.compile(r"(?<!`)`([^`\n]+)`(?!`)")
_MD_LINK = re.compile(r"!?\[[^\]\n]*\]\(\s*<?([^)\s>]+)>?(?:\s+(?:\"[^\"]*\"|'[^']*'))?\s*\)")
_REF_DEF = re.compile(r"^\s{0,3}\[[^\]\n]+\]:\s*<?(\S+?)>?(?:\s|$)", re.M)
_HTML_ATTR = re.compile(r"""\b(?:src|href)\s*=\s*(?:"([^"]*)"|'([^']*)')""", re.I)


def fail(requirement, message):
    return f"FAIL [{requirement}]: {message}"


# --- Markdown helpers ------------------------------------------------------------------------


def split_fences(text):
    """Return (prose lines with fence content replaced by '', [block lines including fences])."""
    prose, blocks = [], []
    block = None
    marker = None
    for line in text.split("\n"):
        fence = _FENCE.match(line)
        if block is None:
            if fence:
                block, marker = [line], fence.group(1)
                prose.append("")
            else:
                prose.append(line)
        else:
            block.append(line)
            prose.append("")
            closing = _CLOSING_FENCE.match(line)
            if closing and closing.group(1)[0] == marker[0] and len(closing.group(1)) >= len(marker):
                blocks.append(block)
                block = None
    if block is not None:
        blocks.append(block)
    return prose, blocks


def headings(text):
    """[(line index, level, title)] for headings outside fenced code."""
    prose, _ = split_fences(text)
    found = []
    for index, line in enumerate(prose):
        match = _HEADING.match(line)
        if match:
            found.append((index, len(match.group(1)), match.group(2)))
    return found


def section(text, predicate):
    """[(title, section text)] for each heading matching predicate, up to the next heading of the
    same or a higher level."""
    lines = text.split("\n")
    found = headings(text)
    result = []
    for position, (index, level, title) in enumerate(found):
        if not predicate(title):
            continue
        end = len(lines)
        for later_index, later_level, _ in found[position + 1 :]:
            if later_level <= level:
                end = later_index
                break
        result.append((title, "\n".join(lines[index:end]), (index, end)))
    return result


def paragraphs(text):
    return [block for block in re.split(r"\n\s*\n", text) if block.strip()]


def link_targets(text):
    """Every Markdown link or image target and HTML src or href, outside code, with lines."""
    prose, _ = split_fences(text)
    targets = []
    for index, line in enumerate(prose):
        plain = _INLINE_CODE.sub(lambda m: " " * len(m.group(0)), line)
        for match in _MD_LINK.finditer(plain):
            targets.append((index + 1, match.group(1)))
        for match in _HTML_ATTR.finditer(plain):
            targets.append((index + 1, match.group(1) if match.group(1) is not None else match.group(2)))
    for match in _REF_DEF.finditer("\n".join(prose)):
        line = "\n".join(prose).count("\n", 0, match.start()) + 1
        targets.append((line, match.group(1)))
    return targets


def is_external(target):
    lowered = target.lower()
    return lowered.startswith(("http://", "https://", "mailto:")) or target.startswith("#")


def resolve(file_path, target):
    """The repository-relative path a relative link points to, or None when it escapes."""
    target = unquote(re.split(r"[#?]", target, maxsplit=1)[0])
    if not target:
        return None
    base = "" if target.startswith("/") else posixpath.dirname(file_path)
    joined = posixpath.normpath(posixpath.join(base, target.lstrip("/")))
    if joined == ".." or joined.startswith("../") or joined.startswith("/"):
        return None
    return "" if joined == "." else joined


def link_findings(requirement, file_path, text, listed):
    """Every relative link of the file resolves to a listed file, or a directory holding one."""
    findings = []
    for line, target in link_targets(text):
        if is_external(target):
            continue
        resolved = resolve(file_path, target)
        ok = resolved is not None and (
            resolved in listed
            or any(path.startswith(resolved + "/") for path in listed)
            or (resolved == "" and listed)
        )
        if not ok:
            findings.append(
                fail(requirement, f"{file_path}:{line}: relative link {target!r} resolves to no "
                     "tracked file or directory")
            )
    return findings


def relative_link_paths(file_path, text):
    paths = set()
    for _, target in link_targets(text):
        if not is_external(target):
            resolved = resolve(file_path, target)
            if resolved is not None:
                paths.add(resolved)
    return paths


# --- R12 -------------------------------------------------------------------------------------


def lock_licenses(lock_data):
    packages = lock_data.get("packages", {}) if isinstance(lock_data, dict) else {}
    return {
        name: packages.get(f"node_modules/{name}", {}).get("license") for name in NPM_PACKAGES
    }


def notices_findings(text, vendor_dirs, licenses):
    """R12. vendor_dirs: the directories under vendor/ at the base. licenses: npm name -> id."""
    if text is None:
        return [fail("R12", "THIRD_PARTY_NOTICES.md does not exist")]
    findings = []
    spans = []
    for directory in vendor_dirs:
        pattern = re.compile(rf"vendor/{re.escape(directory)}(?![\w-])")
        found = section(text, lambda title, p=pattern: bool(p.search(title)))
        if len(found) != 1:
            findings.append(
                fail("R12", f"expected exactly one heading naming vendor/{directory}, "
                     f"found {len(found)}")
            )
            continue
        _, body, span = found[0]
        spans.append(span)
        for needle in VENDOR_FACTS.get(directory, []):
            if needle not in body:
                findings.append(fail("R12", f"the vendor/{directory} section lacks {needle!r}"))
        if directory not in VENDOR_FACTS:
            findings.append(fail("R12", f"vendor/{directory} has no recorded upstream facts"))
    lines = text.split("\n")
    outside = "\n".join(
        line
        for index, line in enumerate(lines)
        if not any(start <= index < end for start, end in spans)
    )
    for name in NPM_PACKAGES:
        value = licenses.get(name)
        if not value:
            findings.append(fail("R12", f"{LOCK} gives no license for {name}"))
        elif not any(name in block and value in block for block in paragraphs(outside)):
            findings.append(
                fail("R12", f"no paragraph outside the vendor sections names {name} with {value}")
            )
    if "IBM Plex" not in outside:
        findings.append(fail("R12", "the notices do not name IBM Plex"))
    if not any("logo.png" in line and "AI-generated" in line for line in outside.split("\n")):
        findings.append(fail("R12", "no line holds both logo.png and AI-generated"))
    for word in ("crates.io", "npm"):
        if word not in outside:
            findings.append(fail("R12", f"the notices do not name {word}"))
    return findings


# --- R13 -------------------------------------------------------------------------------------

_NUMBERING = re.compile(r"^(\d+(?:\.\d+)*\.?)")


def _heading_shape(text):
    return [
        (level, (_NUMBERING.match(title).group(1) if _NUMBERING.match(title) else ""))
        for _, level, title in headings(text)
    ]


def _without_comments(block):
    kept = []
    for line in block:
        cut = line.find("//")
        kept.append((line if cut < 0 else line[:cut]).rstrip())
    return kept


def _inline_spans(text):
    prose, _ = split_fences(text)
    return {match.group(1) for match in _INLINE_CODE.finditer("\n".join(prose))}


def raw_md_findings(base, current):
    if current is None:
        return [fail("R13", "vendor/butterchurn/raw.md does not exist")]
    findings = []
    for number, line in enumerate(current.split("\n"), start=1):
        if re.search("[\\u0400-\\u04ff]", line):
            findings.append(fail("R13", f"raw.md:{number} still holds Cyrillic"))
            break
    if _heading_shape(base) != _heading_shape(current):
        findings.append(
            fail("R13", f"raw.md headings differ in number, level or numbering: "
                 f"{_heading_shape(current)} against {_heading_shape(base)}")
        )
    base_headings, current_headings = headings(base), headings(current)
    if not current_headings or base_headings[0][1:] != current_headings[0][1:]:
        findings.append(fail("R13", "raw.md's first heading differs from the base"))
    _, base_blocks = split_fences(base)
    _, current_blocks = split_fences(current)
    if len(base_blocks) != len(current_blocks):
        findings.append(
            fail("R13", f"raw.md has {len(current_blocks)} code blocks, the base {len(base_blocks)}")
        )
    else:
        for number, (old, new) in enumerate(zip(base_blocks, current_blocks), start=1):
            if _without_comments(old) != _without_comments(new):
                findings.append(fail("R13", f"raw.md code block {number} differs beyond comments"))
    missing = sorted(_inline_spans(base) - _inline_spans(current))
    if missing:
        findings.append(fail("R13", f"raw.md lost inline code spans: {missing}"))
    return findings


def vendor_change_findings(changed, untracked_nonempty):
    findings = []
    others = sorted(set(changed) - {moves.RAW_MD})
    if others:
        findings.append(fail("R13", f"vendor files other than raw.md changed: {others[:10]}"))
    if untracked_nonempty:
        findings.append(fail("R13", f"new files under vendor/: {sorted(untracked_nonempty)[:10]}"))
    return findings


# --- R8 --------------------------------------------------------------------------------------


def license_findings(path, base, current):
    if current is None:
        return [fail("R8", f"{path} does not exist")]
    findings = []
    copyright_lines = [line for line in current.split("\n") if line.startswith("Copyright")]
    if copyright_lines != [OWNER_LINE]:
        findings.append(
            fail("R8", f"{path} must have exactly one Copyright line, {OWNER_LINE!r}, "
                 f"found {copyright_lines}")
        )
    if "syndafloden" in current.lower():
        findings.append(fail("R8", f"{path} still names Syndafloden"))
    findings += [
        fail("R8", finding)
        for finding in moves._license_findings(path, base, current)
    ]
    return findings


def _package_values(data):
    package = data.get("package", {}) if isinstance(data, dict) else {}
    return package.get("license"), package.get("authors"), package.get("repository")


def manifest_findings(manifests, package_json, metadata):
    """R8. manifests: {path: text}; package_json: text; metadata: `cargo metadata` JSON text."""
    findings = []
    expected = ("MIT", AUTHORS, REPOSITORY)
    for name, path in CRATES.items():
        text = manifests.get(path)
        if text is None:
            findings.append(fail("R8", f"{path} does not exist"))
            continue
        try:
            data = tomllib.loads(text)
        except tomllib.TOMLDecodeError as error:
            findings.append(fail("R8", f"{path} does not parse: {error}"))
            continue
        if _package_values(data) != expected:
            findings.append(
                fail("R8", f"{path}: package license, authors and repository are "
                     f"{_package_values(data)}, expected {expected}")
            )
        if path == SRC_TAURI_MANIFEST:
            description = data.get("package", {}).get("description")
            if not isinstance(description, str) or not description.strip() or (
                description == "A Tauri App"
            ):
                findings.append(fail("R8", f"{path}: package.description must be a real one"))
    try:
        package = json.loads(package_json) if package_json is not None else None
    except json.JSONDecodeError as error:
        package = None
        findings.append(fail("R8", f"{moves.PACKAGE_JSON} does not parse: {error}"))
    if not isinstance(package, dict) or package.get("license") != "MIT":
        findings.append(fail("R8", f'{moves.PACKAGE_JSON} must declare "license": "MIT"'))
    try:
        packages = json.loads(metadata).get("packages", []) if metadata else []
    except json.JSONDecodeError:
        packages = []
    by_name = {package.get("name"): package for package in packages}
    for name in CRATES:
        package = by_name.get(name)
        if package is None:
            findings.append(fail("R8", f"cargo metadata does not report package {name}"))
            continue
        values = (package.get("license"), package.get("authors"), package.get("repository"))
        if values != expected:
            findings.append(fail("R8", f"cargo metadata reports {values} for {name}"))
    return findings


# --- R7, R9, R10, R11 ------------------------------------------------------------------------


def _version_after_tool(line):
    return re.search(r"\b(?:Rust|rustc|cargo|Node|npm)\b.{0,20}?(?:\d+\.\d+|\d+\+)", line)


_REPO_URL = re.compile(r"(?:[a-z]+://)?(?:www\.)?github\.com/VladimirKovalevus/[^\s)\]>\"'`<]*", re.I)


def url_findings(text):
    findings = []
    urls = [match.group(0) for match in _REPO_URL.finditer(text)]
    if not urls:
        findings.append(fail("R10", f"README.md has no URL of {REPOSITORY}"))
    for url in urls:
        trimmed = url.rstrip(".,;:!?*_")
        rest = trimmed[len(REPOSITORY) :]
        if not trimmed.startswith(REPOSITORY) or not (
            rest == "" or rest.startswith(("/", ".git", "#"))
        ):
            findings.append(fail("R10", f"README.md links {url!r}, not {REPOSITORY}"))
    return findings


def readme_findings(readme, streaming_readme, listed, reference_hits, deleted_present):
    """R7, R9 (checkable part) and R10.

    listed: repository paths that exist and are tracked or new. reference_hits: lines outside
    vendor/ and scripts/ naming CONTRIBUTING.md or types.md. deleted_present: which of the two
    deleted documents still exist.
    """
    findings = []
    for path in deleted_present:
        findings.append(fail("R7", f"{path} still exists"))
    for hit in reference_hits:
        findings.append(fail("R7", f"a file still refers to a deleted document: {hit}"))
    if streaming_readme is None:
        findings.append(fail("R7", "crates/streaming/README.md does not exist"))
    else:
        for word in ("CONTRIBUTING", "nexus-monorepo"):
            if word in streaming_readme:
                findings.append(fail("R7", f"crates/streaming/README.md still mentions {word}"))
        findings += link_findings("R7", "crates/streaming/README.md", streaming_readme, listed)
    if readme is None:
        return findings + [fail("R9", "README.md does not exist")]

    findings += link_findings("R10", "README.md", readme, listed)
    head = "\n".join(readme.split("\n")[:10])
    images = [
        target
        for _, target in link_targets(head)
        if resolve("README.md", target) == "logo.png"
    ]
    image_syntax = re.search(r"!\[[^\]]*\]\(\s*<?(?:\./)?logo\.png", head) or re.search(
        r"<img\b[^>]*\bsrc\s*=\s*[\"'](?:\./)?logo\.png", head, re.I
    )
    if not (images and image_syntax):
        findings.append(fail("R9", "README.md does not show logo.png within its first 10 lines"))
    _, blocks = split_fences(readme)
    if not any(
        re.match(r"^\s{0,3}(`{3,}|~{3,})\s*mermaid\b", block[0]) and "-->" in "\n".join(block)
        for block in blocks
    ):
        findings.append(fail("R9", "README.md has no mermaid block with an edge (-->)"))
    linked = relative_link_paths("README.md", readme)
    for path in README_LINKS:
        if not any(target == path or target.startswith(path + "/") for target in linked):
            findings.append(fail("R9", f"README.md has no relative link to {path}"))
    for word in README_NAMES + README_BUILD:
        if word not in readme:
            findings.append(fail("R9", f"README.md does not mention {word}"))

    for platform in ("Linux", "Windows", "Android"):
        if not re.search(rf"\b{platform}\b", readme):
            findings.append(fail("R10", f"README.md does not name {platform}"))
    for number, line in enumerate(readme.split("\n"), start=1):
        if re.search(r"macos", line, re.I) and not re.search(r"\bnot\b", line, re.I):
            findings.append(fail("R10", f"README.md:{number} mentions macOS without `not`"))
        for pattern in FORBIDDEN_README:
            if re.search(pattern, line, re.I):
                findings.append(fail("R10", f"README.md:{number} matches forbidden /{pattern}/"))
        if re.search(r"\bAI\b", line):
            findings.append(fail("R10", f"README.md:{number} mentions AI"))
        if _version_after_tool(line):
            findings.append(fail("R10", f"README.md:{number} states a tool version"))
    findings += url_findings(readme)
    return findings


def disclaimer_findings(readme):
    if readme is None:
        return [fail("R11", "README.md does not exist")]
    found = section(readme, lambda title: "disclaimer" in title.lower())
    if not found:
        return [fail("R11", "README.md has no heading containing Disclaimer")]
    findings = []
    body = " ".join(text for _, text, _ in found).lower()
    body = re.sub(r"\s+", " ", body)
    for word in DISCLAIMER_WORDS:
        if word not in body:
            findings.append(fail("R11", f"the disclaimer does not say {word!r}"))
    return findings


# --- R14 -------------------------------------------------------------------------------------


def _logical_lines(run):
    return re.sub(r"\\\n", " ", run).split("\n")


def _has_word(text, word):
    return re.search(rf"(?<![\w.-]){re.escape(word)}(?![\w.-])", text) is not None


def _crate_flags(line):
    return set(re.findall(r"(?:-p|--package)[ =]+([\w.-]+)", line))


def _cargo_lines(run, subcommand):
    pattern = re.compile(rf"\bcargo\s+(?:\+\S+\s+)?{subcommand}\b")
    return [line for line in _logical_lines(run) if pattern.search(line)]


def _is_cargo_step(step):
    return bool(re.search(r"\bcargo\s", step["run"]))


def _steps(job):
    result = []
    for step in job.get("steps") or []:
        if isinstance(step, dict):
            run = step.get("run")
            result.append(
                {
                    "run": run if isinstance(run, str) else "",
                    "uses": step.get("uses"),
                    "env": step.get("env") if isinstance(step.get("env"), dict) else {},
                }
            )
    return result


def _build_job_findings(workflow, job, steps, rust):
    problems = []
    runs = [step["run"] for step in steps]
    if not any(re.search(r"\bapt-get\s+update\b", run) for run in runs):
        problems.append("no apt-get update")
    if not any(
        re.search(r"\bapt-get\b.*\binstall\b", run, re.S)
        and all(_has_word(run, package) for package in APT_PACKAGES)
        for run in runs
    ):
        problems.append(f"no apt-get install naming {', '.join(APT_PACKAGES)}")
    if not any(
        re.search(r"\brustup\s+(?:toolchain\s+install|install)\b", run)
        and _has_word(run, rust)
        and "rustfmt" in run
        and "clippy" in run
        for run in runs
    ):
        problems.append(f"no rustup install of {rust} with rustfmt and clippy")

    cargo_steps = [step for step in steps if _is_cargo_step(step)]
    defaulted = any(
        re.search(rf"\brustup\s+(?:default|override\s+set)\s+{re.escape(rust)}(?![\w.-])", run)
        for run in runs
    )
    env_set = any(
        isinstance(scope, dict) and str(scope.get("RUSTUP_TOOLCHAIN")) == rust
        for scope in (workflow.get("env"), job.get("env"))
    )
    step_env = bool(cargo_steps) and all(
        str(step["env"].get("RUSTUP_TOOLCHAIN")) == rust for step in cargo_steps
    )
    plus_all = bool(cargo_steps) and all(
        re.search(rf"\bcargo\s+\+{re.escape(rust)}\s", line)
        for step in cargo_steps
        for line in _logical_lines(step["run"])
        if re.search(r"\bcargo\s", line)
    )
    if not (defaulted or env_set or step_env or plus_all):
        problems.append(f"cargo does not use the toolchain {rust}")

    first_cargo = next((i for i, step in enumerate(steps) if _is_cargo_step(step)), len(steps))
    if not any(
        "nexus-player-frontend/dist/index.html" in step["run"] for step in steps[:first_cargo]
    ) and not (
        first_cargo < len(steps)
        and "nexus-player-frontend/dist/index.html"
        in steps[first_cargo]["run"].split("cargo")[0]
    ):
        problems.append("nexus-player-frontend/dist/index.html is not created before cargo")

    crates = set(FIRST_PARTY)
    checks = {
        "fmt": lambda line: "--check" in line,
        "clippy": lambda line: all(
            flag in line for flag in ("--all-targets", "--locked", "--no-deps")
        )
        and re.search(r"-D\s*warnings", line),
        "test": lambda line: "--locked" in line,
    }
    for subcommand, extra in checks.items():
        lines = [line for run in runs for line in _cargo_lines(run, subcommand)]
        if not any(_crate_flags(line) == crates and extra(line) for line in lines):
            problems.append(f"no cargo {subcommand} with the three -p flags and its options")
    if not any("python3 scripts/check_file_sizes.py" in run for run in runs):
        problems.append("no python3 scripts/check_file_sizes.py")
    return problems


_SCAN = re.compile(r"\bgitleaks[\"']?\s+dir\b")


def _gitleaks_job_findings(steps, version, digest):
    url = (
        f"https://github.com/gitleaks/gitleaks/releases/download/v{version}/"
        f"gitleaks_{version}_linux_x64.tar.gz"
    )

    def position(predicate):
        for index, step in enumerate(steps):
            match = predicate(step["run"])
            if match is not None:
                return (index, match)
        return None

    download = position(lambda run: run.find(url) if url in run else None)
    verify = position(
        lambda run: run.find(digest)
        if digest in run and re.search(r"\bsha256sum\s+(?:.*\s)?(?:-c|--check)\b", run, re.S)
        else None
    )
    scan = position(
        lambda run: _SCAN.search(run).start() if _SCAN.search(run) else None
    )
    problems = []
    if download is None:
        problems.append(f"no download of {url}")
    if verify is None:
        problems.append(f"no sha256sum check against {digest}")
    if scan is None:
        problems.append("no gitleaks dir scan")
    if problems:
        return problems
    if not download <= verify <= scan:
        problems.append("the gitleaks download, checksum and scan are out of order")
    scan_index, scan_offset = scan
    for index, step in enumerate(steps[: scan_index + 1]):
        run = step["run"] if index < scan_index else step["run"][:scan_offset]
        if re.search(r"\bcargo\s", run):
            problems.append("a cargo step precedes the gitleaks scan")
            break
    return problems


def _all_uses(node):
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "uses":
                yield value
            else:
                yield from _all_uses(value)
    elif isinstance(node, list):
        for item in node:
            yield from _all_uses(item)


def ci_findings(text, rust, gitleaks_version, checksums, checkout_sha):
    """R14. checksums: the text of gitleaks_checksums.txt."""
    if text is None:
        return [fail("R14", f"{moves.CI} does not exist")]
    try:
        import yaml
    except ImportError:
        return [fail("R14", "PyYAML is not installed")]
    try:
        workflow = yaml.safe_load(text)
    except yaml.YAMLError as error:
        return [fail("R14", f"ci.yml does not parse: {error}")]
    if not isinstance(workflow, dict):
        return [fail("R14", "ci.yml is not a mapping")]
    findings = []
    triggers = workflow.get(True, workflow.get("on"))
    if isinstance(triggers, str):
        triggers = [triggers]
    names = set(triggers) if isinstance(triggers, (list, dict)) else set()
    for event in ("push", "pull_request"):
        if event not in names:
            findings.append(fail("R14", f"ci.yml does not trigger on {event}"))

    uses = list(_all_uses(workflow.get("jobs")))
    expected_use = f"actions/checkout@{checkout_sha}"
    if not uses:
        findings.append(fail("R14", f"ci.yml never uses {expected_use}"))
    for value in uses:
        if value != expected_use:
            findings.append(fail("R14", f"ci.yml uses {value!r}, only {expected_use} is allowed"))
    for value in _all_uses({key: val for key, val in workflow.items() if key != "jobs"}):
        findings.append(fail("R14", f"ci.yml uses {value!r} outside its jobs"))

    jobs = workflow.get("jobs")
    if not isinstance(jobs, dict) or not jobs:
        return findings + [fail("R14", "ci.yml has no jobs")]
    for name, job in jobs.items():
        runs_on = job.get("runs-on") if isinstance(job, dict) else None
        if not isinstance(runs_on, str) or not runs_on.startswith("ubuntu-"):
            findings.append(fail("R14", f"job {name} does not run on ubuntu-"))

    digest_match = re.search(
        rf"^([0-9a-f]{{64}})\s+\*?gitleaks_{re.escape(gitleaks_version)}_linux_x64\.tar\.gz$",
        checksums,
        re.M,
    )
    if not digest_match:
        return findings + [fail("R14", "target/t3-ci/gitleaks_checksums.txt has no linux_x64 line")]
    digest = digest_match.group(1)

    build_problems, scan_problems = {}, {}
    for name, job in jobs.items():
        if not isinstance(job, dict):
            continue
        steps = _steps(job)
        build_problems[name] = _build_job_findings(workflow, job, steps, rust)
        scan_problems[name] = _gitleaks_job_findings(steps, gitleaks_version, digest)
    if not any(not problems for problems in build_problems.values()):
        for name, problems in build_problems.items():
            findings.append(fail("R14", f"job {name} is not the build job: {'; '.join(problems)}"))
    if not any(not problems for problems in scan_problems.values()):
        for name, problems in scan_problems.items():
            findings.append(fail("R14", f"job {name} is not the gitleaks job: {'; '.join(problems)}"))

    for word in CI_FORBIDDEN:
        if word in text:
            findings.append(fail("R14", f"ci.yml contains {word!r}"))
    for line in text.split("\n"):
        if re.search(r"\bcargo\b", line):
            for crate in _crate_flags(line) - set(FIRST_PARTY):
                findings.append(fail("R14", f"ci.yml names the crate {crate!r} with -p"))
    return findings


# --- command line ----------------------------------------------------------------------------


def _git(*args):
    return moves._git(*args, check=False)


def _lines(data):
    return [line for line in data.decode(errors="replace").split("\n") if line]


def _listed():
    """Repository paths that exist on disk and are tracked or new, not ignored."""
    output = moves._git("ls-files", "-z", "--cached", "--others", "--exclude-standard").stdout
    return {path for path in moves._nul_separated(output) if (REPO / path).is_file()}


def _ci_value(name):
    path = REPO / CI_DIR / name
    if not path.is_file():
        print(fail("R14", f"{CI_DIR}/{name} does not exist"), file=sys.stderr)
        raise SystemExit(1)
    return path.read_text(encoding="utf-8")


def _cmd_notices(sha):
    listed = moves._git("ls-tree", "-d", "-z", "--name-only", f"{sha}:vendor").stdout
    vendor_dirs = sorted(moves._nul_separated(listed))
    if not vendor_dirs:
        return [fail("R12", "no directory under vendor/ at the base")]
    try:
        lock = json.loads((REPO / LOCK).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [fail("R12", f"cannot read {LOCK}: {error}")]
    return notices_findings(
        moves.current_source("THIRD_PARTY_NOTICES.md"), vendor_dirs, lock_licenses(lock)
    )


def _cmd_raw_md(sha):
    findings = raw_md_findings(moves.base_source(sha, moves.RAW_MD), moves.current_source(moves.RAW_MD))
    changed = _lines(_git("diff", "--name-only", "--no-renames", sha, "--", "vendor").stdout)
    untracked = moves._nul_separated(
        moves._git("ls-files", "-z", "--others", "--exclude-standard", "--", "vendor").stdout
    )
    nonempty = [path for path in untracked if (REPO / path).lstat().st_size != 0]
    return findings + vendor_change_findings(changed, nonempty)


def _cmd_license(sha):
    findings = []
    for path in LICENSE_FILES:
        findings += license_findings(path, moves.base_source(sha, path), moves.current_source(path))
    return findings


def _cmd_metadata(sha):
    findings = []
    for path in (
        "Cargo.lock",
        "nexus-player-frontend/src-tauri/tauri.conf.json",
    ):
        if _git("diff", "--quiet", sha, "--", path).returncode != 0:
            findings.append(fail("R8", f"{path} differs from {sha}"))
    done = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
        cwd=REPO,
        capture_output=True,
    )
    if done.returncode != 0:
        tail = done.stderr.decode(errors="replace")[-2000:]
        findings.append(fail("R8", f"cargo metadata --locked --offline failed: {tail}"))
        metadata = None
    else:
        metadata = done.stdout.decode()
    manifests = {path: moves.current_source(path) for path in CRATES.values()}
    return findings + manifest_findings(
        manifests, moves.current_source(moves.PACKAGE_JSON), metadata
    )


def _cmd_readme(_sha):
    hits = _lines(
        _git(
            "grep", "--untracked", "-n", "-I", "-E", r"CONTRIBUTING[.]md|types[.]md", "--",
            ".", ":!vendor", ":!scripts",
        ).stdout
    )
    deleted = [path for path in moves.DELETABLE if (REPO / path).exists()]
    return readme_findings(
        moves.current_source("README.md"),
        moves.current_source("crates/streaming/README.md"),
        _listed(),
        hits,
        sorted(deleted),
    )


def _cmd_disclaimer(_sha):
    return disclaimer_findings(moves.current_source("README.md"))


def _cmd_ci(_sha):
    return ci_findings(
        moves.current_source(moves.CI),
        _ci_value("rust_version.txt").strip(),
        _ci_value("gitleaks_version.txt").strip(),
        _ci_value("gitleaks_checksums.txt"),
        _ci_value("actions_checkout_v5.0.0.txt").strip(),
    )


COMMANDS = {
    "notices": ("R12", _cmd_notices),
    "raw-md": ("R13", _cmd_raw_md),
    "license": ("R8", _cmd_license),
    "metadata": ("R8", _cmd_metadata),
    "readme": ("R7 R9 R10", _cmd_readme),
    "disclaimer": ("R11", _cmd_disclaimer),
    "ci": ("R14", _cmd_ci),
}


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else argv
    if len(arguments) != 1 or arguments[0] not in COMMANDS:
        print(f"FAIL [usage]: expected one of {', '.join(COMMANDS)}", file=sys.stderr)
        return 2
    label, command = COMMANDS[arguments[0]]
    sha = moves.read_base_sha()
    findings = command(sha)
    for finding in findings:
        print(finding, file=sys.stderr)
    if findings:
        return 1
    print(f"ok [{label}]: {arguments[0]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
