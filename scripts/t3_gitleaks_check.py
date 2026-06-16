#!/usr/bin/env python3
"""T3 R15: gitleaks finds nothing in the files `git archive` would hold for the result.

The tracked and new non-ignored files are copied, paths kept, into a fresh temporary directory,
because `gitleaks dir` also reads ignored files and target/ is huge. Untracked zero-byte files are
skipped, as the tree guard skips them: the sandbox leaves such placeholders, for example an empty
.gitleaks.toml that gitleaks would load as a config with no rules.

Before the real scan, a positive control proves the same gitleaks command detects a GitHub token
assembled at runtime, so nothing secret-shaped is stored in this repository. Every gitleaks run
uses the scanned directory as its working directory.
"""

import json
import os
import random
import shutil
import string
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REQUIRED = ("README.md", "Cargo.lock", "nexus-player-frontend/src-tauri/src/lib.rs")
CONFIG_NAMES = (".gitleaks.toml", ".gitleaksignore")


def fail(message):
    print(f"FAIL [R15]: {message}", file=sys.stderr)
    return 1


def _git_paths(*args):
    done = subprocess.run(["git", *args], cwd=REPO, capture_output=True, check=True)
    return [p for p in done.stdout.decode(errors="surrogateescape").split("\0") if p]


def select_files(listed, tracked, lstat):
    """Return (paths to copy, skipped placeholders) from the listed paths that exist on disk.

    lstat(path) returns an os.stat_result or None when the path does not exist.
    """
    copy, skipped = [], []
    for path in listed:
        info = lstat(path)
        if info is None:
            continue
        if path not in tracked and info.st_size == 0:
            skipped.append(path)
        else:
            copy.append(path)
    return copy, skipped


def _lstat(path):
    try:
        return os.lstat(REPO / path)
    except FileNotFoundError:
        return None


def copy_tree(paths, destination):
    for path in paths:
        source = REPO / path
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            os.symlink(os.readlink(source), target)
        elif source.is_file():
            shutil.copyfile(source, target)
        else:
            raise OSError(f"{path} is not a regular file or a symlink")


def gitleaks_env():
    env = dict(os.environ)
    for key in ("GITLEAKS_CONFIG", "GITLEAKS_CONFIG_TOML"):
        env.pop(key, None)
    return env


def run_gitleaks(binary, directory, report):
    return subprocess.run(
        [
            binary,
            "dir",
            str(directory),
            "--no-banner",
            "--redact",
            "--exit-code",
            "1",
            "--report-format",
            "json",
            "--report-path",
            str(report),
        ],
        cwd=directory,
        env=gitleaks_env(),
        capture_output=True,
        text=True,
    )


def read_report(report):
    try:
        data = json.loads(Path(report).read_text(encoding="utf-8") or "[]")
    except (OSError, json.JSONDecodeError):
        return None
    return data if isinstance(data, list) else None


def control_token():
    alphabet = string.ascii_letters + string.digits
    generator = random.SystemRandom()
    return "ghp_" + "".join(generator.choice(alphabet) for _ in range(36))


def main():
    binary = shutil.which("gitleaks") or "/usr/bin/gitleaks"
    if not os.access(binary, os.X_OK):
        return fail("gitleaks is not installed")
    work = Path(tempfile.mkdtemp(prefix="t3-gitleaks-"))
    try:
        reports = work / "reports"
        reports.mkdir()

        control = work / "control"
        control.mkdir()
        (control / "config.txt").write_text(f"token = {control_token()}\n", encoding="utf-8")
        done = run_gitleaks(binary, control, reports / "control.json")
        findings = read_report(reports / "control.json")
        rules = {item.get("RuleID") for item in findings or [] if isinstance(item, dict)}
        if done.returncode != 1 or "github-pat" not in rules:
            sys.stderr.write(done.stdout[-2000:] + done.stderr[-2000:])
            return fail(
                f"the positive control was not detected (exit {done.returncode}, rules {rules})"
            )
        print("ok [R15]: gitleaks detects the runtime control token as github-pat")

        listed = _git_paths("ls-files", "-z", "--cached", "--others", "--exclude-standard")
        tracked = set(_git_paths("ls-files", "-z", "--cached"))
        paths, skipped = select_files(listed, tracked, _lstat)
        copy = work / "tree"
        copy.mkdir()
        try:
            copy_tree(paths, copy)
        except OSError as error:
            return fail(f"cannot copy the tree: {error}")
        copied = [p for p in copy.rglob("*") if p.is_file() or p.is_symlink()]
        if len(copied) != len(paths):
            return fail(f"copied {len(copied)} files, expected {len(paths)}")
        for path in REQUIRED:
            if not (copy / path).is_file():
                return fail(f"{path} is missing from the copy")
        configs = [p.relative_to(copy).as_posix() for p in copied if p.name in CONFIG_NAMES]
        if configs:
            return fail(f"gitleaks would read these as config or allowlist: {configs}")

        done = run_gitleaks(binary, copy, reports / "tree.json")
        findings = read_report(reports / "tree.json")
        if done.returncode != 0 or findings != []:
            sys.stderr.write(done.stdout[-2000:] + done.stderr[-2000:])
            for item in (findings or [])[:20]:
                print(f"  {item.get('RuleID')}: {item.get('File')}:{item.get('StartLine')}",
                      file=sys.stderr)
            return fail(f"gitleaks reports findings or failed (exit {done.returncode})")
        print(
            f"ok [R15]: gitleaks finds nothing in {len(paths)} files "
            f"({len(skipped)} empty placeholders skipped)"
        )
        return 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
