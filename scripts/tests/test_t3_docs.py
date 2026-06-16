"""T3 R7-R14: the document checks on one compliant fixture set, and one minimal mutation per rule
that fails it. Inputs come from in-memory fixtures and git objects at the T3 base only."""

import json
import re
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))
sys.path.insert(0, str(REPO / "scripts/tests"))

import t3_doc_fixtures as fx  # noqa: E402
import t3_docs_check as docs  # noqa: E402
import t3_moves_check as moves  # noqa: E402


def base_sha():
    return (REPO / moves.BASE_FILE).read_text(encoding="utf-8").strip()


def git(*args):
    return subprocess.run(["git", *args], cwd=REPO, capture_output=True, check=True).stdout


def git_show(path):
    return git("show", f"{base_sha()}:{path}").decode("utf-8")


def base_listed():
    """Tracked files at the base, plus the new files T3 adds."""
    listed = set(git("ls-tree", "-r", "-z", "--name-only", base_sha()).decode().split("\0"))
    return (listed - {"", "types.md", "CONTRIBUTING.md"}) | {
        "THIRD_PARTY_NOTICES.md",
        ".github/workflows/ci.yml",
    }


def vendor_dirs():
    out = git("ls-tree", "-d", "-z", "--name-only", f"{base_sha()}:vendor").decode()
    return sorted(name for name in out.split("\0") if name)


class Case(unittest.TestCase):
    def assert_clean(self, findings):
        self.assertEqual(findings, [])

    def assert_fails(self, findings, requirement, contains=None):
        self.assertTrue(findings, "expected a finding")
        self.assertTrue(all(f.startswith(f"FAIL [{requirement}]") for f in findings), findings)
        if contains:
            self.assertTrue(any(contains in f for f in findings), findings)


class Notices(Case):
    """R12."""

    def check(self, text, licenses=None):
        return docs.notices_findings(text, vendor_dirs(), licenses or fx.LOCK_LICENSES)

    def test_vendor_dirs_at_the_base(self):
        self.assertEqual(
            vendor_dirs(), ["Symphonia", "bandcamp", "butterchurn", "rsoundcloud", "symphonia-adapters"]
        )

    def test_holders_match_the_base_license_files(self):
        for directory, (path, holder) in docs.HOLDERS.items():
            with self.subTest(directory=directory):
                self.assertRegex(git_show(path), rf"Copyright \(c\) [0-9-]+ {re.escape(holder)}\n")
                self.assertIn(holder, docs.VENDOR_FACTS[directory])

    def test_licenses_match_the_base_license_files(self):
        self.assertIn("Mozilla Public License Version 2.0", git_show("vendor/Symphonia/symphonia/LICENSE"))
        self.assertIn("Apache License", git_show("vendor/symphonia-adapters/LICENSE-APACHE"))
        self.assertIn("MIT License", git_show("vendor/symphonia-adapters/LICENSE-MIT"))

    def test_lock_licenses_from_the_base_lock(self):
        lock = json.loads(git_show(docs.LOCK))
        self.assertEqual(docs.lock_licenses(lock), fx.LOCK_LICENSES)

    def test_compliant(self):
        self.assert_clean(self.check(fx.notices()))

    def test_missing_file(self):
        self.assert_fails(self.check(None), "R12")

    def test_each_vendor_fact_is_required(self):
        for directory, needles in docs.VENDOR_FACTS.items():
            for needle in needles:
                with self.subTest(directory=directory, needle=needle):
                    sections = dict(fx.NOTICE_SECTIONS)
                    sections[directory] = sections[directory].replace(needle, "~")
                    self.assert_fails(self.check(fx.notices(sections)), "R12", needle)

    def test_fact_only_in_another_section_fails(self):
        sections = dict(fx.NOTICE_SECTIONS)
        sections["bandcamp"] = sections["bandcamp"].replace("bb4864c", "~")
        sections["butterchurn"] += "\nbb4864c\n"
        self.assert_fails(self.check(fx.notices(sections)), "R12", "bb4864c")

    def test_missing_section(self):
        sections = dict(fx.NOTICE_SECTIONS)
        del sections["rsoundcloud"]
        self.assert_fails(self.check(fx.notices(sections)), "R12", "vendor/rsoundcloud")

    def test_duplicate_heading(self):
        sections = dict(fx.NOTICE_SECTIONS)
        sections["again"] = "## vendor/bandcamp, again\n\nx\n"
        self.assert_fails(self.check(fx.notices(sections)), "R12", "vendor/bandcamp")

    def test_presets_license_must_match_the_lock(self):
        self.assert_fails(
            self.check(fx.notices(), dict(fx.LOCK_LICENSES, **{"butterchurn-presets": "ISC"})),
            "R12",
            "butterchurn-presets",
        )

    def test_font_license_must_be_named(self):
        tail = fx.NOTICES_TAIL.replace(" (OFL-1.1)", "")
        self.assert_fails(self.check(fx.notices(tail=tail)), "R12", "ibm-plex-mono")

    def test_font_license_must_match_the_lock(self):
        licenses = dict(fx.LOCK_LICENSES, **{"@fontsource/ibm-plex-mono": "MIT"})
        text = fx.notices().replace("butterchurn-presets (MIT).\n", "butterchurn-presets (MIT).\n\n")
        self.assert_clean(self.check(text))
        found = self.check(text, licenses)
        self.assert_fails(found, "R12", "ibm-plex-mono")

    def test_ibm_plex_named(self):
        tail = fx.NOTICES_TAIL.replace("IBM Plex", "Plex")
        self.assert_fails(self.check(fx.notices(tail=tail)), "R12", "IBM Plex")

    def test_logo_line(self):
        tail = fx.NOTICES_TAIL.replace("is AI-generated", "is generated")
        self.assert_fails(self.check(fx.notices(tail=tail)), "R12", "logo.png")

    def test_registries(self):
        for word in ("crates.io", "npm"):
            with self.subTest(word=word):
                tail = fx.NOTICES_TAIL.replace(word, "~")
                tail = tail.replace("@fontsource", "@fontsource")
                self.assert_fails(self.check(fx.notices(tail=tail)), "R12")

    def test_other_facts_inside_a_vendor_section_do_not_count(self):
        sections = dict(fx.NOTICE_SECTIONS)
        sections["butterchurn"] += "\nThe logo, logo.png, is AI-generated.\n"
        tail = fx.NOTICES_TAIL.replace("- The logo, logo.png, is AI-generated.\n", "")
        self.assert_fails(self.check(fx.notices(sections, tail)), "R12", "logo.png")


class RawMd(Case):
    """R13."""

    def setUp(self):
        self.base = git_show(moves.RAW_MD)

    def check(self, text):
        return docs.raw_md_findings(self.base, text)

    def test_base_is_cyrillic(self):
        self.assert_fails(self.check(self.base), "R13", "Cyrillic")

    def test_compliant_translation(self):
        self.assert_clean(self.check(fx.RAW_MD))

    def test_cyrillic_left(self):
        self.assert_fails(self.check(fx.RAW_MD.replace("Initialization", "\u0418\u043d\u0438\u0446\u0438\u0430\u043b\u0438\u0437\u0430\u0446\u0438\u044f")), "R13")

    def test_heading_removed(self):
        text = fx.RAW_MD.replace("### If your backend", "If your backend")
        self.assert_fails(self.check(text), "R13", "headings")

    def test_heading_level_changed(self):
        self.assert_fails(self.check(fx.RAW_MD.replace("## 3. Calling", "### 3. Calling")), "R13")

    def test_heading_numbering_changed(self):
        self.assert_fails(self.check(fx.RAW_MD.replace("## 3. Calling", "## Calling")), "R13")

    def test_first_heading_changed(self):
        text = fx.RAW_MD.replace("# Butterchurn Raw Audio Interface", "# Raw audio for Butterchurn")
        self.assert_fails(self.check(text), "R13", "first heading")

    def test_code_changed(self):
        self.assert_fails(self.check(fx.RAW_MD.replace("height: 600", "height: 601")), "R13", "code block")

    def test_code_comment_translated_is_fine(self):
        self.assertIn("// Fill with random data for testing", fx.RAW_MD)
        self.assert_clean(self.check(fx.RAW_MD.replace("// Fill with random data", "// Random data")))

    def test_code_block_added(self):
        self.assert_fails(self.check(fx.RAW_MD + "\n```\nx\n```\n"), "R13", "code blocks")

    def test_inline_span_lost(self):
        text = fx.RAW_MD.replace("`performance.now()`", "performance.now()")
        self.assert_fails(self.check(text), "R13", "performance.now()")

    def test_heading_inside_code_does_not_count(self):
        text = fx.RAW_MD.replace("const mockData", "# not a heading\nconst mockData")
        self.assert_fails(self.check(text), "R13", "code block")
        self.assertEqual(docs._heading_shape(text), docs._heading_shape(self.base))

    def test_vendor_changes(self):
        self.assert_clean(docs.vendor_change_findings([moves.RAW_MD], []))
        self.assert_fails(docs.vendor_change_findings([moves.RAW_MD, "vendor/bandcamp/src/lib.rs"], []), "R13")
        self.assert_fails(docs.vendor_change_findings([], ["vendor/new.txt"]), "R13")


class Licenses(Case):
    """R8, LICENSE part."""

    def setUp(self):
        self.base = git_show("LICENSE")
        self.good = self.base.replace("Copyright (c) 2026 Syndafloden", docs.OWNER_LINE)

    def test_base_names_the_old_holder(self):
        self.assert_fails(docs.license_findings("LICENSE", self.base, self.base), "R8")
        self.assertEqual(self.base, git_show("crates/streaming/LICENSE"))

    def test_compliant(self):
        for path in docs.LICENSE_FILES:
            with self.subTest(path=path):
                base = git_show(path)
                current = base.replace("Copyright (c) 2026 Syndafloden", docs.OWNER_LINE)
                self.assert_clean(docs.license_findings(path, base, current))

    def test_wrong_name(self):
        text = self.base.replace("Syndafloden", "V. Kovalev")
        self.assert_fails(docs.license_findings("LICENSE", self.base, text), "R8")

    def test_two_copyright_lines(self):
        text = self.good.replace(docs.OWNER_LINE, docs.OWNER_LINE + "\nCopyright (c) 2026 Other")
        self.assert_fails(docs.license_findings("LICENSE", self.base, text), "R8")

    def test_old_name_elsewhere(self):
        text = self.good + "syndafloden\n"
        self.assert_fails(docs.license_findings("LICENSE", self.base, text), "R8", "Syndafloden")

    def test_other_line_changed(self):
        text = self.good.replace("MIT License", "The MIT License")
        self.assert_fails(docs.license_findings("LICENSE", self.base, text), "R8")

    def test_missing(self):
        self.assert_fails(docs.license_findings("LICENSE", self.base, None), "R8")


class Metadata(Case):
    """R8, manifests, package.json and cargo metadata."""

    def setUp(self):
        self.manifests = {}
        for name, path in docs.CRATES.items():
            description = "Desktop music player built with Tauri" if name == "nexus-player-frontend" else None
            self.manifests[path] = fx.with_package_keys(git_show(path), description)
        package = json.loads(git_show(moves.PACKAGE_JSON))
        package["license"] = "MIT"
        self.package = json.dumps(package, indent=2)
        self.metadata = fx.metadata_json()

    def check(self, manifests=None, package=None, metadata=None):
        return docs.manifest_findings(
            manifests or self.manifests, package or self.package, metadata or self.metadata
        )

    def test_compliant(self):
        self.assert_clean(self.check())

    def test_compliant_manifests_pass_the_tree_guard(self):
        current = dict(self.manifests, **{moves.PACKAGE_JSON: self.package})
        found = moves.guard_findings(list(current), set(), git_show, current.get)
        self.assertEqual(found, [])

    def test_base_fails(self):
        manifests = {path: git_show(path) for path in docs.CRATES.values()}
        self.assert_fails(self.check(manifests, git_show(moves.PACKAGE_JSON), "{}"), "R8")

    def test_each_key_is_required(self):
        for path in docs.CRATES.values():
            for key in ("license", "authors", "repository"):
                with self.subTest(path=path, key=key):
                    manifests = dict(self.manifests)
                    manifests[path] = "\n".join(
                        line for line in manifests[path].split("\n") if not line.startswith(f"{key} =")
                    )
                    self.assert_fails(self.check(manifests), "R8", path)

    def test_wrong_authors(self):
        path = docs.CRATES["streaming"]
        manifests = dict(self.manifests)
        manifests[path] = manifests[path].replace("vladkoval805@gmail.com", "someone@example.com")
        self.assert_fails(self.check(manifests), "R8")

    def test_template_description(self):
        path = docs.SRC_TAURI_MANIFEST
        manifests = dict(self.manifests)
        manifests[path] = manifests[path].replace("Desktop music player built with Tauri", "A Tauri App")
        self.assert_fails(self.check(manifests), "R8", "description")

    def test_package_json_license(self):
        package = json.loads(self.package)
        package["license"] = "ISC"
        self.assert_fails(self.check(package=json.dumps(package)), "R8", "package.json")

    def test_cargo_metadata_values(self):
        metadata = fx.metadata_json({"streaming": {"repository": None}})
        self.assert_fails(self.check(metadata=metadata), "R8", "streaming")

    def test_cargo_metadata_missing_package(self):
        metadata = json.dumps({"packages": [{"name": "nexus-player-backend"}]})
        self.assert_fails(self.check(metadata=metadata), "R8")


class Readme(Case):
    """R7, R9 (checkable part), R10."""

    def setUp(self):
        self.listed = base_listed()

    def check(self, readme=fx.README, streaming=fx.STREAMING_README, hits=(), deleted=()):
        return docs.readme_findings(readme, streaming, self.listed, list(hits), list(deleted))

    def test_compliant(self):
        self.assert_clean(self.check())

    def test_base_readme_fails(self):
        found = self.check(git_show("README.md"), git_show("crates/streaming/README.md"))
        self.assertTrue(any(f.startswith("FAIL [R7]") for f in found), found)
        self.assertTrue(any(f.startswith("FAIL [R9]") for f in found), found)
        self.assertTrue(any(f.startswith("FAIL [R10]") for f in found), found)

    def test_deleted_documents_present(self):
        self.assert_fails(self.check(deleted=["types.md"]), "R7", "types.md")

    def test_reference_hit(self):
        self.assert_fails(self.check(hits=["README.md:3:see CONTRIBUTING.md"]), "R7")

    def test_streaming_readme_words(self):
        for text in ("CONTRIBUTING", "nexus-monorepo"):
            with self.subTest(text=text):
                self.assert_fails(self.check(streaming=fx.STREAMING_README + text + "\n"), "R7", text)

    def test_streaming_readme_broken_link(self):
        streaming = fx.STREAMING_README + "[x](../nexus-player/docs.md)\n"
        self.assert_fails(self.check(streaming=streaming), "R7", "relative link")

    def test_logo_required_in_the_first_ten_lines(self):
        readme = fx.README.replace('<p align="center"><img src="logo.png" alt="Nexus Player logo" width="160"></p>\n', "")
        self.assert_fails(self.check(readme), "R9", "logo.png")
        moved = "\n" * 12 + fx.README
        self.assert_fails(self.check(moved), "R9", "logo.png")

    def test_markdown_logo(self):
        readme = fx.README.replace(
            '<p align="center"><img src="logo.png" alt="Nexus Player logo" width="160"></p>', "![Nexus Player](./logo.png)"
        )
        self.assert_clean(self.check(readme))

    def test_mermaid_required(self):
        self.assert_fails(self.check(fx.README.replace("```mermaid", "```text")), "R9", "mermaid")
        self.assert_fails(self.check(fx.README.replace("-->", "---")), "R9", "mermaid")

    def test_each_code_link_required(self):
        for path in docs.README_LINKS:
            with self.subTest(path=path):
                readme = re.sub(rf"\]\({re.escape(path)}[^)]*\)", "]", fx.README)
                self.assertNotEqual(readme, fx.README)
                self.assert_fails(self.check(readme), "R9", path)

    def test_each_name_and_package_required(self):
        for word in docs.README_NAMES + docs.README_BUILD:
            with self.subTest(word=word):
                readme = fx.README.replace(word, "~")
                self.assertTrue(any(word in f for f in self.check(readme)))

    def test_platforms(self):
        for platform in ("Linux", "Windows", "Android"):
            with self.subTest(platform=platform):
                self.assert_fails(self.check(fx.README.replace(platform, "~")), "R10", platform)

    def test_macos_without_not(self):
        readme = fx.README.replace("macOS is not built or tested.", "macOS works too.")
        self.assert_fails(self.check(readme), "R10", "macOS")

    def test_each_forbidden_term(self):
        samples = {
            r"youtube": "YouTube", r"\bvk\b": "VK", r"vk[.]com": "vk.com", r"vkontakte": "VKontakte",
            r"planned": "Planned", r"coming soon": "Coming soon", r"roadmap": "Roadmap",
            r"\bwip\b": "WIP", r"work in progress": "Work in progress",
            r"nexus-monorepo": "nexus-monorepo", r"syndafloden": "Syndafloden",
            r"goalflow": "goalflow", r"scripts/acceptance": "scripts/acceptance",
            r"\bphase [0-9]": "Phase 2", r"tauri[.]app/v1": "tauri.app/v1", r"msrv": "MSRV",
            r"rust-version": "rust-version", r"artificial intelligence": "artificial intelligence",
            r"\bllm": "LLMs", r"\bgpt\b": "GPT", r"claude": "Claude", r"anthropic": "Anthropic",
            r"openai": "OpenAI", r"copilot": "Copilot", r"hand-written": "hand-written",
            r"handwritten": "handwritten", r"\bby hand\b": "by hand",
            r"human-written": "human-written", r"bilingual": "bilingual",
            r"multilingual": "multilingual", r"i18n": "i18n", r"locali[sz]": "localized",
            r"translat": "translations", r"english": "English", r"russian": "Russian",
        }
        self.assertEqual(set(samples), set(docs.FORBIDDEN_README))
        for pattern, sample in samples.items():
            with self.subTest(pattern=pattern):
                readme = fx.README.replace("## Features\n", f"## Features\n\n{sample} note.\n")
                self.assert_fails(self.check(readme), "R10", pattern)

    def test_ai_word(self):
        readme = fx.README.replace("## Features\n", "## Features\n\nNo AI here.\n")
        self.assert_fails(self.check(readme), "R10", "AI")

    def test_tool_versions(self):
        for line in ("Rust 1.70 or newer", "Node 18+", "cargo (at least 1.78)", "npm v10.2", "rustc 1.98.0"):
            with self.subTest(line=line):
                readme = fx.README.replace("## Features\n", f"## Features\n\n{line}\n")
                self.assert_fails(self.check(readme), "R10", "version")

    def test_repository_urls(self):
        for url in (
            "https://github.com/VladimirKovalevus/other",
            "http://github.com/VladimirKovalevus/nexus-player",
            "https://github.com/VladimirKovalevus/nexus-player-old",
            "github.com/VladimirKovalevus/nexus-player",
        ):
            with self.subTest(url=url):
                readme = fx.README + f"\nSee {url}.\n"
                self.assert_fails(self.check(readme), "R10", "links")
        for url in (
            "https://github.com/VladimirKovalevus/nexus-player.git",
            "https://github.com/VladimirKovalevus/nexus-player/issues",
            "https://github.com/VladimirKovalevus/nexus-player#readme",
        ):
            with self.subTest(url=url):
                self.assert_clean(self.check(fx.README + f"\nSee <{url}>.\n"))

    def test_repository_url_required(self):
        readme = fx.README.replace("Source: https://github.com/VladimirKovalevus/nexus-player", "")
        self.assert_fails(self.check(readme), "R10", "no URL")

    def test_broken_relative_links(self):
        for link in (
            "[x](docs/missing.md)", "[x](CONTRIBUTING.md)", "![x](screenshot.png)",
            '<a href="types.md">x</a>', "[x](../outside.md)", "[x]: missing.md",
            "[x](nexus-player-frontend/dist/index.html)",
        ):
            with self.subTest(link=link):
                self.assert_fails(self.check(fx.README + f"\n{link}\n"), "R10", "relative link")

    def test_relative_link_forms_that_resolve(self):
        for link in (
            "[x](LICENSE#license)", "[x](./LICENSE?plain=1)", "[x](nexus-player-backend/)",
            "[x](#architecture)", "[x](mailto:a@b.c)", "[x](https://example.com/y)",
            "`[x](missing.md)`", "[x](crates/streaming/README%2Emd)",
        ):
            with self.subTest(link=link):
                self.assert_clean(self.check(fx.README + f"\n{link}\n"))

    def test_link_inside_a_code_block_is_ignored(self):
        self.assert_clean(self.check(fx.README + "\n```md\n[x](missing.md)\n```\n"))


class Disclaimer(Case):
    """R11."""

    def test_compliant(self):
        self.assert_clean(docs.disclaimer_findings(fx.README))

    def test_missing_heading(self):
        self.assert_fails(docs.disclaimer_findings(fx.README.replace("## Disclaimer", "## Notes")), "R11")

    def test_each_word_required(self):
        for word in docs.DISCLAIMER_WORDS:
            with self.subTest(word=word):
                start = fx.README.index("## Disclaimer")
                end = fx.README.index("## License")
                section = re.sub(re.escape(word), "~", fx.README[start:end], flags=re.I)
                readme = fx.README[:start] + section + fx.README[end:]
                self.assert_fails(docs.disclaimer_findings(readme), "R11", word)

    def test_words_outside_the_section_do_not_count(self):
        start = fx.README.index("## Disclaimer")
        end = fx.README.index("## License")
        readme = (
            fx.README[:start] + "## Disclaimer\n\nSee below.\n\n" + fx.README[end:]
            + "\n" + fx.README[start:end].replace("## Disclaimer", "Fine print:")
        )
        self.assert_fails(docs.disclaimer_findings(readme), "R11")


class Ci(Case):
    """R14."""

    def check(self, text=fx.CI_YML):
        return docs.ci_findings(text, fx.RUST, fx.GITLEAKS, fx.CHECKSUMS, fx.CHECKOUT)

    def mutate(self, old, new):
        self.assertIn(old, fx.CI_YML)
        return self.check(fx.CI_YML.replace(old, new, 1))

    def test_compliant(self):
        self.assert_clean(self.check())

    def test_missing_file_and_bad_yaml(self):
        self.assert_fails(self.check(None), "R14")
        self.assert_fails(self.check("on: [push\n"), "R14")

    def test_triggers(self):
        self.assert_fails(self.mutate("  pull_request:\n", ""), "R14", "pull_request")
        self.assert_fails(self.mutate("on:\n  push:\n  pull_request:\n", "on: [pull_request]\n"), "R14", "push")
        self.assert_clean(self.mutate("on:\n  push:\n  pull_request:\n", "on: [push, pull_request]\n"))

    def test_other_action(self):
        self.assert_fails(
            self.mutate("      - name: Format\n", "      - uses: actions/cache@v4\n      - name: Format\n"),
            "R14", "actions/cache",
        )

    def test_unpinned_checkout(self):
        self.assert_fails(self.mutate(f"actions/checkout@{fx.CHECKOUT}", "actions/checkout@v5"), "R14")

    def test_runner(self):
        self.assert_fails(self.mutate("runs-on: ubuntu-24.04", "runs-on: macos-14"), "R14", "ubuntu")

    def test_each_apt_package(self):
        for package in docs.APT_PACKAGES:
            with self.subTest(package=package):
                text = fx.CI_YML.replace(f" {package} ", " ").replace(f" {package}\n", "\n")
                self.assertNotEqual(text, fx.CI_YML)
                self.assert_fails(self.check(text), "R14", "apt-get install")

    def test_apt_update(self):
        self.assert_fails(self.mutate("          sudo apt-get update\n", ""), "R14", "apt-get update")

    def test_toolchain_version(self):
        self.assert_fails(self.mutate(f"toolchain install {fx.RUST}", "toolchain install stable"), "R14", "rustup")

    def test_toolchain_components(self):
        self.assert_fails(self.mutate("--component rustfmt,clippy", "--component rustfmt"), "R14", "rustup")

    def test_toolchain_selection(self):
        self.assert_fails(self.mutate(f"      RUSTUP_TOOLCHAIN: {fx.RUST}\n", "      CARGO_TERM_COLOR: always\n"), "R14", "toolchain")
        text = fx.CI_YML.replace(f"    env:\n      RUSTUP_TOOLCHAIN: {fx.RUST}\n", "").replace(
            "--component rustfmt,clippy", f"--component rustfmt,clippy\n          rustup default {fx.RUST}"
        )
        self.assert_clean(self.check(text.replace("run: rustup toolchain", "run: |\n          rustup toolchain")))

    def test_dist_before_cargo(self):
        self.assert_fails(self.mutate("printf '<!doctype html>\\n' > nexus-player-frontend/dist/index.html", "true"), "R14", "dist")

    def test_fmt(self):
        self.assert_fails(self.mutate("cargo fmt -p nexus-player-backend -p nexus-player-frontend -p streaming -- --check", "cargo fmt -p nexus-player-backend -p nexus-player-frontend -- --check"), "R14", "fmt")
        self.assert_fails(self.mutate("-p streaming -- --check", "-p streaming"), "R14", "fmt")

    def test_clippy(self):
        for flag in ("--all-targets ", "--locked --no-deps", "-- -D warnings"):
            with self.subTest(flag=flag):
                text = fx.CI_YML.replace(
                    "cargo clippy -p nexus-player-backend -p nexus-player-frontend -p streaming --all-targets --locked --no-deps -- -D warnings",
                    "cargo clippy -p nexus-player-backend -p nexus-player-frontend -p streaming --all-targets --locked --no-deps -- -D warnings".replace(flag, "--locked " if flag == "--locked --no-deps" else ""),
                )
                self.assertNotEqual(text, fx.CI_YML)
                self.assert_fails(self.check(text), "R14", "clippy")

    def test_tests(self):
        self.assert_fails(self.mutate("-p streaming --locked\n", "-p streaming\n"), "R14", "test")

    def test_file_sizes(self):
        self.assert_fails(self.mutate("python3 scripts/check_file_sizes.py", "true"), "R14", "check_file_sizes")

    def test_gitleaks_version(self):
        self.assert_fails(self.check(fx.CI_YML.replace(f"v{fx.GITLEAKS}/gitleaks_{fx.GITLEAKS}", "v8.0.0/gitleaks_8.0.0")), "R14", "download")

    def test_gitleaks_checksum(self):
        self.assert_fails(self.mutate(fx.DIGEST, "cd" * 32), "R14", "sha256sum")
        self.assert_fails(self.mutate("| sha256sum -c -", "> sums.txt"), "R14", "sha256sum")

    def test_gitleaks_scan(self):
        self.assert_fails(self.mutate('"$RUNNER_TEMP/gitleaks" dir .', '"$RUNNER_TEMP/gitleaks" detect'), "R14", "gitleaks dir")

    def test_cargo_before_the_scan(self):
        text = fx.CI_YML.replace(
            "      - name: Scan the checkout\n",
            "      - name: Build first\n        run: cargo build -p streaming --locked\n      - name: Scan the checkout\n",
        )
        self.assert_fails(self.check(text), "R14", "cargo step precedes")

    def test_scan_in_the_build_job_after_cargo_fails(self):
        start = fx.CI_YML.index("  gitleaks:\n")
        end = fx.CI_YML.index("  build:\n")
        steps = fx.CI_YML[fx.CI_YML.index("      - name: Install gitleaks", start):end]
        text = fx.CI_YML[:start] + fx.CI_YML[end:] + steps
        self.assert_fails(self.check(text), "R14", "cargo step precedes")

    def test_forbidden_words(self):
        for word in docs.CI_FORBIDDEN:
            with self.subTest(word=word):
                text = fx.CI_YML + f"\n# {word}\n"
                self.assert_fails(self.check(text), "R14", word)

    def test_foreign_crate(self):
        self.assert_fails(self.mutate("-p streaming --locked\n", "-p streaming -p rsoundcloud --locked\n"), "R14")


if __name__ == "__main__":
    unittest.main()
