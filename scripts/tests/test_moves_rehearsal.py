"""T2 R2, R5: one honest split per family goes through the real comparison, in memory.

moves_check.py is locked before anyone implements the splits, so a false finding on an honest
move has to surface here. The cuts use line ranges found by simple text rules, independent of
the moves_check.py segmenter, so that a parser bug cannot hide inside its own cut. Each family
also has must-fail variants, so the rehearsal cannot pass vacuously.
"""

import re
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))

from check_file_sizes import LIMIT, counted_lines  # noqa: E402
from moves_check import BASE_FILE, FAMILIES, compare_sources  # noqa: E402

PRIVATE_FN = re.compile(r"^    ((?:async |const |unsafe )*fn )", re.M)


def base_text(family):
    sha = (REPO / BASE_FILE).read_text(encoding="utf-8").strip()
    return subprocess.run(
        ["git", "show", f"{sha}:{FAMILIES[family]}"], cwd=REPO, capture_output=True, check=True
    ).stdout.decode("utf-8")


class Source:
    """A base file whose lines can be cut out by text rules. Line 0 is the first line."""

    def __init__(self, text):
        self.lines = text.split("\n")
        self.gone = set()

    def _find(self, pattern, start=0):
        regex = re.compile(pattern)
        for index in range(start, len(self.lines)):
            if regex.search(self.lines[index]):
                return index
        raise AssertionError(f"no line matches {pattern!r}")

    def _take(self, first, last):
        overlap = self.gone & set(range(first, last + 1))
        if overlap:
            raise AssertionError(f"lines {sorted(overlap)[:3]} were already cut")
        self.gone.update(range(first, last + 1))
        return self.lines[first : last + 1]

    def _item_start(self, index, indent):
        while re.match(rf"^{indent}(///|#\[)", self.lines[index - 1]):
            index -= 1
        return index

    def take_method(self, name, after=None):
        """A four-space fn, with its docs and attributes, through the next four-space `}`."""
        start = self._find(after) if after else 0
        pattern = rf"^    (pub(\(super\))? )?(const )?(async )?(unsafe )?fn {name}\b"
        index = self._find(pattern, start)
        last = index
        while self.lines[last] != "    }":
            last += 1
        return self._take(self._item_start(index, "    "), last) + [""]

    def take_top(self, pattern):
        """A top-level item, with its docs and attributes, through the next column-zero `}`."""
        index = self._find(pattern)
        last = index
        while self.lines[last] != "}":
            last += 1
        return self._take(self._item_start(index, ""), last) + [""]

    def take_section(self, first_pattern, next_pattern):
        """What is left from one section comment up to the next one."""
        first = self._find(first_pattern)
        last = self._find(next_pattern, first + 1) - 1
        left = [index for index in range(first, last + 1) if index not in self.gone]
        self.gone.update(left)
        return [self.lines[index] for index in left]

    def rest(self):
        return "\n".join(
            line for index, line in enumerate(self.lines) if index not in self.gone
        )


def widen(lines):
    return [PRIVATE_FN.sub(r"    pub(super) \1", line) for line in lines]


def child_impl(header, lines):
    return header + "\n" + "\n".join(lines).rstrip() + "\n}\n"


def split_playback(with_play_local=False):
    root = FAMILIES["playback"]
    source = Source(base_text("playback"))
    lines = []
    for pattern in (
        r"^pub struct HlsReaderAdapter\b",
        r"^impl HlsReaderAdapter\b",
        r"^impl Read for HlsReaderAdapter\b",
        r"^impl Seek for HlsReaderAdapter\b",
        r"^impl symphonia::core::io::MediaSource for HlsReaderAdapter\b",
        r"^impl MediaSource for HlsReaderAdapter\b",
    ):
        lines += source.take_top(pattern)
    header = (
        "use super::lock_recover;\n"
        "use crate::audio::types::MediaSource;\n"
        "use std::io::{Read, Seek, SeekFrom};\n"
        "use std::sync::{Arc, Mutex};\n"
        "use streaming::multi_downloader::source::HlsSource;\n\n"
    )
    files = {f"{root[:-3]}/hls_reader.rs": header + "\n".join(lines).rstrip() + "\n"}
    declarations = "mod hls_reader;\npub use hls_reader::HlsReaderAdapter;\n"
    if with_play_local:
        moved = widen(source.take_method("play_local"))
        files[f"{root[:-3]}/local_source.rs"] = child_impl(
            "use super::PlaybackEngine;\n\nimpl PlaybackEngine {", moved
        )
        declarations += "mod local_source;\n"
    files[root] = declarations + source.rest()
    return files


def split_queue():
    root = FAMILIES["queue"]
    source = Source(base_text("queue"))
    moved = widen(source.take_top(r"^impl SearchQueue\b"))
    child = (
        "use super::{CurrentPosition, NextResult, QueueEntry, SearchQueue, SearchQueueItem};\n"
        "use crate::audio::types::Track;\n\n" + "\n".join(moved).rstrip() + "\n"
    )
    parent = source.rest()
    marker = "mod search_queue_tests {\n"
    if marker not in parent:
        raise AssertionError("the inline test module must stay in the parent")
    parent = parent.replace(marker, marker + "    use std::collections::VecDeque as _;\n", 1)
    return {f"{root[:-3]}/search_queue.rs": child, root: "mod search_queue;\n" + parent}


def split_hls():
    root = FAMILIES["hls"]
    source = Source(base_text("hls"))
    readers = []
    for name in ("HlsBufferReader", "HlsSequentialReader"):
        for pattern in (
            rf"^pub struct {name}\b",
            rf"^impl {name}\b",
            rf"^impl Read for {name}\b",
            rf"^impl Seek for {name}\b",
            rf"^impl MediaSource for {name}\b",
        ):
            readers += source.take_top(pattern)
    persistence = []
    for name in (
        "save_to_file",
        "save_to_folder",
        "load_from_folder",
        "extract_bitrate",
        "extract_duration",
    ):
        persistence += widen(source.take_method(name))
    return {
        f"{root[:-3]}/readers.rs": (
            "use super::HlsBuffer;\nuse std::sync::Arc;\n\n" + "\n".join(readers).rstrip() + "\n"
        ),
        f"{root[:-3]}/persistence.rs": child_impl(
            "use super::HlsBuffer;\n\nimpl HlsBuffer {", persistence
        ),
        root: (
            "mod persistence;\nmod readers;\n"
            "pub use readers::{HlsBufferReader, HlsSequentialReader};\n" + source.rest()
        ),
    }


def split_manager():
    root = FAMILIES["manager"]
    source = Source(base_text("manager"))
    moved = []
    for name in ("handle_command", "event_loop", "process_task", "evict_if_needed"):
        moved += widen(source.take_method(name))
    return {
        f"{root[:-3]}/worker.rs": child_impl(
            "use super::DownloadManager;\n\nimpl DownloadManager {", moved
        ),
        root: "mod worker;\n" + source.rest(),
    }


def split_state():
    root = FAMILIES["state"]
    prefix = root[:-3]
    source = Source(base_text("state"))

    playback = []
    for name in (
        "play_next",
        "play_current",
        "preload_next_track",
        "play_previous",
        "play",
        "handle_track_finished",
        "begin_exclusive_op",
        "play_cancellable",
    ):
        playback += source.take_method(name)

    search = []
    for name in (
        "fill_search_queue",
        "load_search_to_queue",
        "load_search_and_play",
        "load_search_queue",
        "search_and_play",
        "expand_search_container",
        "fetch_more_search_results",
        "play_current_search",
    ):
        search += source.take_method(name)

    # Both of these sections carry their section comments along. The comments of the groups
    # above, such as "--- Playback Control ---", stay behind in the parent.
    loading = source.take_section(r"// --- Queue Management ---", r"// --- Cancellation mechanism")
    for name in (
        "load_and_play_library_page_at_track",
        "load_and_play_library_page",
        "fill_context_queue",
        "load_context_to_queue",
        "load_context_and_play",
        "load_playlist_to_queue",
        "load_playlist_and_play",
    ):
        loading += source.take_method(name)

    playlists = []
    for name in (
        "create_playlist",
        "get_all_playlists",
        "get_playlist",
        "get_playlist_with_items",
        "add_playlist_to_playlist",
        "remove_playlist_from_playlist",
        "add_track_to_playlist",
        "remove_track_from_playlist",
        "delete_playlist",
        "create_playlist_with_tracks",
        "create_playlist_from_container",
    ):
        playlists += source.take_method(name)
    library = source.take_section(r"// --- Library Proxies", r"// --- Liked Tracks ---")

    return {
        f"{prefix}/playback_control.rs": child_impl(
            "use super::AppState;\n\nimpl AppState {", widen(playback)
        ),
        f"{prefix}/search_loading.rs": child_impl("impl super::AppState {", widen(search)),
        f"{prefix}/queue_loading.rs": child_impl("impl super::AppState {", widen(loading)),
        f"{prefix}/library_proxies.rs": "mod playlists;\n\n"
        + child_impl("use super::AppState;\n\nimpl AppState {", widen(library)),
        f"{prefix}/library_proxies/playlists.rs": child_impl(
            "impl super::super::AppState {", widen(playlists)
        ),
        root: (
            "mod library_proxies;\nmod playback_control;\nmod queue_loading;\nmod search_loading;\n"
            + source.rest()
        ),
    }


SPLITS = {
    "playback": split_playback,
    "queue": split_queue,
    "hls": split_hls,
    "manager": split_manager,
    "state": split_state,
}

# For each family, one moved member and the file holding it, and one child's `mod` line.
DELETABLE = {
    "playback": ("hls_reader.rs", "read"),
    "queue": ("search_queue.rs", "advance"),
    "hls": ("persistence.rs", "extract_bitrate"),
    "manager": ("worker.rs", "evict_if_needed"),
    "state": ("search_loading.rs", "expand_search_container"),
}
MOD_LINES = {
    "playback": "mod hls_reader;\n",
    "queue": "mod search_queue;\n",
    "hls": "mod persistence;\n",
    "manager": "mod worker;\n",
    "state": "mod queue_loading;\n",
}


def delete_method(text, name):
    source = Source(text)
    source.take_method(name)
    return source.rest()


def compare(family, current):
    root = FAMILIES[family]
    return compare_sources({root: base_text(family)}, current, family)


class HonestSplits(unittest.TestCase):
    def assert_within_limit(self, current):
        for path, text in current.items():
            self.assertLessEqual(counted_lines(text), LIMIT, path)

    def test_each_family_split_gives_no_finding(self):
        for family, split in SPLITS.items():
            with self.subTest(family=family):
                current = split()
                self.assertGreater(len(current), 1)
                self.assertEqual(compare(family, current), [])

    def test_each_family_split_is_within_the_limit(self):
        for family, split in SPLITS.items():
            with self.subTest(family=family):
                self.assert_within_limit(split())

    def test_playback_variant_moving_a_private_method(self):
        current = split_playback(with_play_local=True)
        child = current[f"{FAMILIES['playback'][:-3]}/local_source.rs"]
        self.assertIn("pub(super) fn play_local", child)
        self.assertEqual(compare("playback", current), [])
        self.assert_within_limit(current)

    def test_state_split_has_the_required_shape(self):
        current = split_state()
        prefix = FAMILIES["state"][:-3]
        root = FAMILIES["state"]
        children = [path for path in current if path != root]
        self.assertGreaterEqual(len(children), 3)
        self.assertIn("use super::AppState;\n\nimpl AppState {", current[f"{prefix}/playback_control.rs"])
        self.assertIn("impl super::AppState {", current[f"{prefix}/search_loading.rs"])
        self.assertIn(f"{prefix}/library_proxies/playlists.rs", current)
        self.assertIn("pub(super) async fn play(", current[f"{prefix}/playback_control.rs"])
        self.assertIn("pub(super) async fn fill_cursor_queue(", current[f"{prefix}/queue_loading.rs"])
        # Methods holding a body-level `use` moved, section comments both moved and stayed.
        search = current[f"{prefix}/search_loading.rs"]
        self.assertEqual(search.count("        use crate::streaming::types::SearchResult as SR;"), 3)
        self.assertEqual(search.count("        use crate::streaming::types::ContentType;"), 1)
        self.assertNotIn("as SR;", current[root])
        self.assertIn("// --- Playback Control ---", current[root])
        self.assertIn("// --- Queue Management ---", current[f"{prefix}/queue_loading.rs"])
        self.assertIn("// --- Library Proxies", current[f"{prefix}/library_proxies.rs"])

    def test_queue_split_keeps_types_and_tests_in_the_parent(self):
        parent = split_queue()[FAMILIES["queue"]]
        for name in ("pub enum ActiveQueue", "pub struct CursorQueue", "pub struct SearchQueue"):
            self.assertIn(name, parent)
        self.assertIn("mod search_queue_tests {", parent)
        self.assertNotIn("impl SearchQueue", parent)
        child = split_queue()[f"{FAMILIES['queue'][:-3]}/search_queue.rs"]
        self.assertIn("pub(super) fn regenerate_shuffle", child)

    def test_playback_split_keeps_the_engine_whole(self):
        current = split_playback()
        parent = current[FAMILIES["playback"]]
        self.assertIn("pub struct PlaybackEngine", parent)
        self.assertIn("fn create_stream", parent)
        self.assertNotIn("struct HlsReaderAdapter", parent)


class MustFail(unittest.TestCase):
    def test_deleting_one_moved_member_is_a_finding(self):
        for family, split in SPLITS.items():
            file_name, member = DELETABLE[family]
            with self.subTest(family=family):
                current = split()
                path = f"{FAMILIES[family][:-3]}/{file_name}"
                current[path] = delete_method(current[path], member)
                self.assertTrue(compare(family, current))

    def test_removing_one_mod_line_is_a_finding(self):
        for family, split in SPLITS.items():
            with self.subTest(family=family):
                current = split()
                root = FAMILIES[family]
                self.assertIn(MOD_LINES[family], current[root])
                current[root] = current[root].replace(MOD_LINES[family], "", 1)
                found = compare(family, current)
                self.assertTrue(any("no `mod` declares" in finding for finding in found), found)

    def test_removing_the_nested_mod_line_is_a_finding(self):
        current = split_state()
        path = f"{FAMILIES['state'][:-3]}/library_proxies.rs"
        current[path] = current[path].replace("mod playlists;\n", "", 1)
        found = compare("state", current)
        self.assertTrue(any("playlists.rs: no `mod` declares" in finding for finding in found), found)

    def test_real_peek_upcoming_ids_moved_to_an_undeclared_file(self):
        root = FAMILIES["queue"]
        source = Source(base_text("queue"))
        moved = source.take_method("peek_upcoming_ids", after=r"^impl ActiveQueue\b")
        self.assertIn("    pub fn peek_upcoming_ids(&self, limit: usize) -> Vec<u64> {", moved)
        path = f"{root[:-3]}/active_queue_peek.rs"
        current = {
            root: source.rest(),
            path: child_impl("use super::ActiveQueue;\n\nimpl ActiveQueue {", moved),
        }
        found = compare("queue", current)
        self.assertEqual(len(found), 1, found)
        self.assertIn(f"{path}: no `mod` declares", found[0])

    def test_making_a_moved_method_plain_pub_is_a_finding(self):
        current = split_manager()
        path = f"{FAMILIES['manager'][:-3]}/worker.rs"
        current[path] = current[path].replace("pub(super) async fn event_loop", "pub async fn event_loop")
        self.assertTrue(compare("manager", current))


if __name__ == "__main__":
    unittest.main()
