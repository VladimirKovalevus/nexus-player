"""T2 R1 and T3 R1: self-tests for check_file_sizes.py, on base sources and synthetic files."""

import ast
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts/check_file_sizes.py"
sys.path.insert(0, str(REPO / "scripts"))

from check_file_sizes import LIMIT, counted_lines, offenders, total_lines  # noqa: E402

T2_BASE_COUNTS = {
    "nexus-player-backend/src/state/state.rs": 2025,
    "nexus-player-backend/src/playlist/playback_queue.rs": 905,
    "nexus-player-backend/src/audio/playback.rs": 806,
    "crates/streaming/src/state/manager.rs": 913,
    "crates/streaming/src/buffers/hls.rs": 932,
    "nexus-player-backend/src/library/service.rs": 760,
}
T3_BASE_COUNTS = {
    "nexus-player-frontend/src-tauri/src/commands.rs": 1693,
    "nexus-player-frontend/src-tauri/src/dto.rs": 874,
}


def base_text(base_file, path):
    sha = (REPO / "scripts/acceptance" / base_file).read_text(encoding="utf-8").strip()
    return subprocess.run(
        ["git", "show", f"{sha}:{path}"], cwd=REPO, capture_output=True, check=True
    ).stdout.decode("utf-8")


class BaseCounts(unittest.TestCase):
    def test_counts_on_the_t2_base_sources(self):
        for path, expected in T2_BASE_COUNTS.items():
            with self.subTest(path=path):
                self.assertEqual(counted_lines(base_text("T2_BASE_SHA", path)), expected)

    def test_counts_on_the_t3_base_sources(self):
        for path, expected in T3_BASE_COUNTS.items():
            with self.subTest(path=path):
                self.assertEqual(counted_lines(base_text("T3_BASE_SHA", path)), expected)


class CountingRule(unittest.TestCase):
    def test_final_newline_adds_no_phantom_line(self):
        self.assertEqual(total_lines("a\nb\n"), 2)

    def test_text_after_the_last_newline_counts(self):
        self.assertEqual(total_lines("a\nb"), 2)

    def test_empty_file_has_no_lines(self):
        self.assertEqual(total_lines(""), 0)

    def test_blank_lines_and_comments_outside_the_span_count(self):
        self.assertEqual(counted_lines("// a\n\nfn x() {}\n"), 3)

    def test_span_runs_from_the_attribute_through_the_closing_brace(self):
        source = "fn x() {}\n#[cfg(test)]\nmod tests {\n    fn y() {}\n}\nfn z() {}\n"
        self.assertEqual(counted_lines(source), 2)

    def test_span_of_a_semicolon_item(self):
        source = "fn x() {}\n#[cfg(test)]\nmod wire_format_tests;\nfn z() {}\n"
        self.assertEqual(counted_lines(source), 2)

    def test_cfg_all_test_span(self):
        source = "#[cfg(all(test, unix))]\nmod t {\n}\nfn z() {}\n"
        self.assertEqual(counted_lines(source), 1)

    def test_two_test_blocks(self):
        block = "#[cfg(test)]\nmod t {\n}\n"
        self.assertEqual(counted_lines("fn a() {}\n" + block + "fn b() {}\n" + block), 2)

    def test_cfg_test_inside_a_string_or_comment_is_no_block(self):
        source = '// #[cfg(test)]\nconst A: &str = "#[cfg(test)]";\nfn b() {\n}\n'
        self.assertEqual(counted_lines(source), 4)

    def test_cfg_test_inside_a_raw_string_is_no_block(self):
        source = 'const A: &str = r#"#[cfg(test)]"#;\nfn b() {\n}\n'
        self.assertEqual(counted_lines(source), 3)

    def test_brace_inside_a_test_string_does_not_end_the_span(self):
        source = '#[cfg(test)]\nmod t {\n    const A: &str = "}";\n}\nfn b() {}\n'
        self.assertEqual(counted_lines(source), 1)

    def test_brace_char_and_lifetime_inside_a_test_item(self):
        source = "#[cfg(test)]\nmod t {\n    fn f<'a>(_: &'a str) -> char {\n        '}'\n    }\n}\nfn b() {}\n"
        self.assertEqual(counted_lines(source), 1)


class Limit(unittest.TestCase):
    def test_synthetic_file_over_the_limit_fails(self):
        self.assertEqual(offenders({"a.rs": "fn a() {}\n" * (LIMIT + 1)}), [("a.rs", LIMIT + 1)])

    def test_file_exactly_at_the_limit_passes(self):
        self.assertEqual(offenders({"a.rs": "fn a() {}\n" * LIMIT}), [])

    def test_over_the_limit_only_through_a_cfg_test_block_passes(self):
        tests = "#[cfg(test)]\nmod tests {\n" + "    fn y() {}\n" * LIMIT + "}\n"
        self.assertEqual(offenders({"a.rs": "fn a() {}\n" * 10 + tests}), [])

    def test_cfg_not_test_code_counts(self):
        block = "#[cfg(not(test))]\nmod real {\n" + "    fn y() {}\n" * LIMIT + "}\n"
        self.assertEqual(offenders({"a.rs": block}), [("a.rs", LIMIT + 3)])

    def test_the_limit_is_800(self):
        self.assertEqual(LIMIT, 800)


class Standalone(unittest.TestCase):
    """T3 R1: CI and the public repository keep the script after the run tooling is deleted."""

    def setUp(self):
        self.source = SCRIPT.read_text(encoding="utf-8")
        self.tree = ast.parse(self.source)

    def test_imports_only_the_standard_library(self):
        names = set()
        for node in ast.walk(self.tree):
            if isinstance(node, ast.Import):
                names.update(alias.name.split(".")[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom):
                self.assertEqual(node.level, 0, "relative import")
                names.add(node.module.split(".")[0])
        self.assertTrue(names)
        self.assertEqual(names - set(sys.stdlib_module_names), set())

    def test_mentions_no_run_tooling_and_no_sha(self):
        for word in ("moves_check", "rust_scan", "BASE_SHA", "T3_EXCEPTIONS", "sys.path"):
            with self.subTest(word=word):
                self.assertNotIn(word, self.source)
        self.assertIsNone(re.search(r"\b[0-9a-f]{40}\b", self.source))

    def test_exports_limit_and_counted_lines(self):
        top = {
            node.name
            for node in self.tree.body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef))
        }
        assigned = {
            target.id
            for node in self.tree.body
            if isinstance(node, ast.Assign)
            for target in node.targets
            if isinstance(target, ast.Name)
        }
        self.assertIn("counted_lines", top)
        self.assertIn("LIMIT", assigned)
        function = next(
            node
            for node in self.tree.body
            if isinstance(node, ast.FunctionDef) and node.name == "counted_lines"
        )
        self.assertEqual([arg.arg for arg in function.args.args], ["text"])

    def test_finds_the_root_from_its_own_location(self):
        self.assertIn("__file__", self.source)


class IsolatedTree(unittest.TestCase):
    """A copy of the script alone in a fresh tree, run with `python3 -I` from its root."""

    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root)
        (self.root / "scripts").mkdir()
        shutil.copy(SCRIPT, self.root / "scripts/check_file_sizes.py")
        self.write("nexus-player-backend/src/lib.rs", "fn a() {}\n" * 10)
        self.write("nexus-player-frontend/src-tauri/src/lib.rs", "fn a() {}\n" * 10)
        self.write("nexus-player-frontend/src-tauri/src/commands.rs", "fn a() {}\n" * 10)
        self.write("crates/streaming/src/lib.rs", "fn a() {}\n" * 10)

    def write(self, relative, text):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def run_script(self, *arguments):
        return subprocess.run(
            [sys.executable, "-I", "scripts/check_file_sizes.py", *arguments],
            cwd=self.root,
            capture_output=True,
            text=True,
        )

    def test_small_files_pass(self):
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_an_801_line_commands_rs_fails_so_no_exception_survives(self):
        self.write("nexus-player-frontend/src-tauri/src/commands.rs", "fn a() {}\n" * 801)
        done = self.run_script()
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("commands.rs has 801 lines", done.stderr)

    def test_an_801_line_dto_rs_fails(self):
        self.write("nexus-player-frontend/src-tauri/src/dto.rs", "fn a() {}\n" * 801)
        self.assertNotEqual(self.run_script().returncode, 0)

    def test_test_files_and_target_and_gen_are_skipped_without_arguments(self):
        big = "fn a() {}\n" * 900
        self.write("nexus-player-frontend/src-tauri/src/wire_format_tests.rs", big)
        self.write("nexus-player-backend/tests/big.rs", big)
        self.write("crates/streaming/target/debug/build/out.rs", big)
        self.write("nexus-player-frontend/src-tauri/gen/android/x.rs", big)
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_a_missing_root_fails(self):
        shutil.rmtree(self.root / "crates")
        self.assertNotEqual(self.run_script().returncode, 0)

    def test_a_root_without_rs_files_fails(self):
        shutil.rmtree(self.root / "crates/streaming/src")
        (self.root / "crates/streaming/README.md").write_text("x\n", encoding="utf-8")
        self.assertNotEqual(self.run_script().returncode, 0)

    def test_path_mode_checks_test_named_files(self):
        self.write("nexus-player-frontend/src-tauri/src/commands/queue_tests.rs", "fn a() {}\n" * 801)
        done = self.run_script("nexus-player-frontend/src-tauri/src/commands")
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("queue_tests.rs has 801 lines", done.stderr)

    def test_path_mode_single_file(self):
        self.write("nexus-player-frontend/src-tauri/src/dto.rs", "fn a() {}\n" * 801)
        self.assertNotEqual(
            self.run_script("nexus-player-frontend/src-tauri/src/dto.rs").returncode, 0
        )
        self.assertEqual(
            self.run_script("nexus-player-frontend/src-tauri/src/commands.rs").returncode, 0
        )

    def test_path_mode_skips_a_nonexistent_argument_next_to_an_existing_one(self):
        done = self.run_script(
            "nexus-player-frontend/src-tauri/src/commands.rs",
            "nexus-player-frontend/src-tauri/src/commands",
        )
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_path_mode_with_no_rs_file_fails(self):
        done = self.run_script("nexus-player-frontend/src-tauri/src/nothing")
        self.assertNotEqual(done.returncode, 0)


if __name__ == "__main__":
    unittest.main()
