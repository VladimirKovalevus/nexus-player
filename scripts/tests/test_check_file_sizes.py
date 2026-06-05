"""T2 R1: self-tests for check_file_sizes.py, on the base sources and on synthetic files."""

import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))

import check_file_sizes  # noqa: E402
from check_file_sizes import LIMIT, counted_lines, offenders, total_lines  # noqa: E402

BASE_COUNTS = {
    "nexus-player-backend/src/state/state.rs": 2025,
    "nexus-player-backend/src/playlist/playback_queue.rs": 905,
    "nexus-player-backend/src/audio/playback.rs": 806,
    "crates/streaming/src/state/manager.rs": 913,
    "crates/streaming/src/buffers/hls.rs": 932,
    "nexus-player-backend/src/library/service.rs": 760,
}


def base_text(path):
    sha = (REPO / "scripts/acceptance/T2_BASE_SHA").read_text(encoding="utf-8").strip()
    return subprocess.run(
        ["git", "show", f"{sha}:{path}"], cwd=REPO, capture_output=True, check=True
    ).stdout.decode("utf-8")


class BaseCounts(unittest.TestCase):
    def test_counts_on_the_base_sources(self):
        for path, expected in BASE_COUNTS.items():
            with self.subTest(path=path):
                self.assertEqual(counted_lines(base_text(path)), expected)


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

    def test_two_test_blocks(self):
        block = "#[cfg(test)]\nmod t {\n}\n"
        self.assertEqual(counted_lines("fn a() {}\n" + block + "fn b() {}\n" + block), 2)

    def test_cfg_test_inside_a_string_or_comment_is_no_block(self):
        source = '// #[cfg(test)]\nconst A: &str = "#[cfg(test)]";\nfn b() {\n}\n'
        self.assertEqual(counted_lines(source), 4)

    def test_brace_inside_a_test_string_does_not_end_the_span(self):
        source = '#[cfg(test)]\nmod t {\n    const A: &str = "}";\n}\nfn b() {}\n'
        self.assertEqual(counted_lines(source), 1)


class Limit(unittest.TestCase):
    def over_limit(self):
        return "fn a() {}\n" * (LIMIT + 1)

    def test_synthetic_file_over_the_limit_fails(self):
        self.assertEqual(offenders({"a.rs": self.over_limit()}), [("a.rs", LIMIT + 1)])

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

    def test_exceptions_are_exactly_the_two_t3_files(self):
        self.assertEqual(
            check_file_sizes.T3_EXCEPTIONS,
            {
                "nexus-player-frontend/src-tauri/src/commands.rs",
                "nexus-player-frontend/src-tauri/src/dto.rs",
            },
        )


class CommandLine(unittest.TestCase):
    def test_unknown_family_is_refused(self):
        done = subprocess.run(
            [sys.executable, str(REPO / "scripts/check_file_sizes.py"), "--family", "nope"],
            capture_output=True,
        )
        self.assertNotEqual(done.returncode, 0)


if __name__ == "__main__":
    unittest.main()
