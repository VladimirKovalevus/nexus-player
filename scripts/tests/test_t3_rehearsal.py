"""T3 R2, R3, R5: one honest split of commands.rs and dto.rs goes through the real checks.

t3_moves_check.py is locked before anyone implements the splits, so a false finding on an honest
move has to surface here. The cuts use line ranges found by simple text rules, independent of
the moves_check segmenter: from a column-0 item line, with the doc comments, plain comments and
attributes directly above it, through the next column-0 `}`. The splits call the same functions
the command line uses. Must-fail variants keep the rehearsal from passing vacuously.

In memory only: reads git objects, writes no file. `build_*_split` are also used to write the
splits into a scratch clone at author time.
"""

import re
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))

from check_file_sizes import LIMIT, counted_lines  # noqa: E402
from moves_check import compare_sources  # noqa: E402
import t3_commands_check as commands_check  # noqa: E402
import t3_dto_check as dto_check  # noqa: E402
import t3_moves_check as moves  # noqa: E402

COMMANDS = moves.FAMILIES["commands"]
DTO = moves.FAMILIES["dto"]
COMMANDS_DIR = COMMANDS[: -len(".rs")]
DTO_DIR = DTO[: -len(".rs")]

ITEM_START = re.compile(r"^(#\[tauri::command\]|pub struct |pub enum |impl\b|pub\(crate\) fn |fn )")
ABOVE = re.compile(r"^(///|//|#\[)")

COMMAND_CHILDREN = {
    "library": [
        "get_all_tracks", "get_cached_tracks", "get_cached_albums", "get_track",
        "search_tracks", "add_track_from_file", "delete_track", "cleanup_library",
        "delete_cached_track",
    ],
    "local_sources": [
        "get_local_library", "scan_local_library", "import_android_media",
        "get_music_sources", "add_music_source", "remove_music_source",
    ],
    "playlists": [
        "get_all_playlists", "get_playlist_with_tracks", "add_playlist_to_playlist",
        "remove_playlist_from_playlist", "create_playlist", "delete_playlist",
        "create_playlist_from_container", "create_playlist_with_tracks",
        "add_track_to_playlist", "remove_track_from_playlist", "reorder_playlist_items",
    ],
    "artwork": ["resolve_artwork", "clear_artwork_cache"],
    "queue": [
        "add_to_queue", "remove_from_queue", "clear_queue", "get_queue",
        "load_playlist_to_queue", "load_library_page_to_queue", "load_library_page",
        "load_and_play_library_page_at_track", "play_library_track",
        "load_and_play_library_tracks", "load_and_play_local_tracks",
        "load_playlist_and_play", "load_and_play_library_page", "play_at_index",
        "play_next", "play_previous", "set_repeat_mode", "toggle_shuffle",
    ],
    "transport": [
        "pause", "resume", "stop", "seek", "set_volume", "get_audio_state", "retry_playback",
    ],
    "radio": ["start_radio", "stop_radio"],
    "queue_state": [
        "get_queue_state", "radio_mode_label", "build_queue_state_dto", "handle_search",
        "expand_queue_container", "play_queue_item",
    ],
    "dialogs": ["select_sync_file", "select_audio_file", "select_image_file", "select_audio_files"],
    "provider_search": ["search_streaming", "handle_streaming_track", "direct_link_resolve"],
    "app_config": [
        "get_app_config", "update_app_config", "update_provider_auth",
        "submit_challenge_result", "open_challenge_window",
    ],
    "export": ["save_track_to_file"],
    "history": [
        "get_play_history", "clear_play_history", "toggle_liked", "reorder_liked_tracks",
    ],
    "downloads": ["list_download_tasks", "remove_download_task"],
    "sync": [
        "export_sync", "import_sync", "telegram_config_path", "telegram_get_config",
        "telegram_save_config", "telegram_check_config", "telegram_push", "telegram_pull",
    ],
    "eq": ["get_eq_settings", "set_eq_settings", "get_eq_presets"],
}

DTO_CHILDREN = {
    "track": ["TrackDto", "TrackSourceDto", "TrackStateDto"],
    "library": ["CachedAlbumDto", "CleanupResultDto", "LibraryPageDto", "ResolvedArtworkDto"],
    "playlist": [
        "PlaylistDto", "PlaylistItemDto", "PlaylistWithItemsDto", "CreatePlaylistRequest",
        "CreatePlaylistWithTracksRequest",
    ],
    "queue": [
        "ContainerMetaDto", "QueueEntryDto", "CurrentPositionDto", "SearchContextDto",
        "QueueStateDto",
    ],
    "audio_state": ["AudioStateDto", "WaveformDto", "BufferedRangeDto"],
    "history": ["PlayHistoryEntryDto"],
    "search": [
        "SearchResultDto", "StreamingTrackDto", "StreamingPlaylistDto", "StreamingAlbumDto",
        "StreamingChannelDto", "hash_str_to_i64",
    ],
    "downloads": ["TaskInfoDto"],
    "eq": ["EqSettingsDto", "EqPresetDto"],
}


def base_text(path):
    sha = (REPO / moves.BASE_FILE).read_text(encoding="utf-8").strip()
    return subprocess.run(
        ["git", "show", f"{sha}:{path}"], cwd=REPO, capture_output=True, check=True
    ).stdout.decode("utf-8")


class Item:
    def __init__(self, first, last, lines):
        self.first, self.last, self.lines = first, last, lines
        self.text = "\n".join(lines[first : last + 1])

    def name(self):
        """The fn, struct or enum name, or for an impl the first DTO type its header names."""
        head = self.text
        header = re.search(r"^impl\b[^{]*", head, re.M)
        if header:
            dto = re.search(r"\b([A-Z]\w*Dto|Create\w+Request)\b", header.group(0))
            return dto.group(1) if dto else None
        fn = re.search(r"^(?:pub(?:\(crate\))? )?(?:async )?fn (\w+)", head, re.M)
        if fn:
            return fn.group(1)
        return re.search(r"^pub (?:struct|enum) (\w+)", head, re.M).group(1)


def cut_items(text):
    """Return (items, leftover line indexes) by the column-0 rule."""
    lines = text.split("\n")
    items, taken = [], set()
    index = 0
    while index < len(lines):
        if index not in taken and ITEM_START.match(lines[index]):
            first = index
            while first > 0 and first - 1 not in taken and ABOVE.match(lines[first - 1]):
                first -= 1
            last = index
            while lines[last] != "}":
                last += 1
            items.append(Item(first, last, lines))
            taken.update(range(first, last + 1))
            index = last + 1
        else:
            index += 1
    leftover = [i for i in range(len(lines)) if i not in taken]
    return items, leftover, lines


def module_uses(lines):
    """The module-level `use` lines of the base, joined over their continuation lines."""
    result, current = [], None
    for line in lines:
        if current is not None:
            current.append(line)
            if line.rstrip().endswith(";"):
                result.append("\n".join(current))
                current = None
        elif re.match(r"^use ", line):
            if line.rstrip().endswith(";"):
                result.append(line)
            else:
                current = [line]
    return result


def build_commands_split(base_commands, base_lib, children=None):
    """Return ({path: text} of the commands family, new lib.rs text)."""
    children = children or COMMAND_CHILDREN
    owner = {name: child for child, names in children.items() for name in names}
    items, leftover, lines = cut_items(base_commands)
    uses = module_uses(lines)
    bodies = {child: [] for child in children}
    for item in items:
        bodies[owner[item.name()]].append(item.text)
    root = [lines[i] for i in leftover]
    declarations = [f"pub(crate) mod {child};" for child in children]
    radio_child = owner["radio_mode_label"]
    position = next(i for i, line in enumerate(root) if line.startswith("type Result<T>"))
    root[position:position] = declarations + [
        "",
        f"pub(crate) use {radio_child}::radio_mode_label;",
        "",
    ]
    files = {COMMANDS: "\n".join(root)}
    for child, parts in bodies.items():
        header = ["use super::Result;"] + uses + [""]
        files[f"{COMMANDS_DIR}/{child}.rs"] = "\n".join(header) + "\n" + "\n\n".join(parts) + "\n"
    lib = re.sub(
        r"^(\s*)commands::(\w+),$",
        lambda m: f"{m.group(1)}commands::{owner[m.group(2)]}::{m.group(2)},",
        base_lib,
        flags=re.M,
    )
    return files, lib


def build_dto_split(base_dto, children=None):
    """Return {path: text} of the dto family."""
    children = children or DTO_CHILDREN
    owner = {name: child for child, names in children.items() for name in names}
    items, leftover, lines = cut_items(base_dto)
    uses = [use for use in module_uses(lines) if not use.startswith("pub use")]
    bodies = {child: [] for child in children}
    for item in items:
        bodies[owner[item.name()]].append(item.text)
    root = [lines[i] for i in leftover]
    exports = []
    for child, names in children.items():
        types = [name for name in names if name[0].isupper()]
        exports.append(f"pub use {child}::{{{', '.join(types)}}};")
    position = next(i for i, line in enumerate(root) if line.startswith("pub use "))
    root[position + 1 : position + 1] = (
        [""]
        + [f"mod {child};" for child in children]
        + [""]
        + exports
        + ["pub(crate) use search::hash_str_to_i64;"]
    )
    test_use = next(i for i, line in enumerate(root) if line.startswith("    use super::*;"))
    root.insert(test_use + 1, "    use super::track::TrackSourceDto as _;")
    files = {DTO: "\n".join(root)}
    for child, parts in bodies.items():
        header = ["use super::*;"] + uses + [""]
        files[f"{DTO_DIR}/{child}.rs"] = "\n".join(header) + "\n" + "\n\n".join(parts) + "\n"
    return files


def with_declaration(lib):
    return lib.replace("mod events;\n", "mod events;\n#[cfg(test)]\nmod wire_format_tests;\n", 1)


class Base:
    @classmethod
    def setUpClass(cls):
        cls.commands = base_text(COMMANDS)
        cls.dto = base_text(DTO)
        cls.lib = base_text(moves.LIB)
        cls.events = base_text(moves.EVENTS)
        cls.names, problems = commands_check.base_findings(cls.commands, cls.lib)
        assert not problems, problems


class CommandsSplit(Base, unittest.TestCase):
    def setUp(self):
        self.files, self.new_lib = build_commands_split(self.commands, self.lib)

    def findings(self, files=None, lib=None):
        files = self.files if files is None else files
        lib = self.new_lib if lib is None else lib
        return (
            compare_sources({COMMANDS: self.commands}, files, "commands")
            + moves.vague_name_findings(files)
            + commands_check.split_findings(self.names, files, lib)
            + moves.lib_findings(self.lib, lib)
        )

    def child(self, name):
        return f"{COMMANDS_DIR}/{name}.rs"

    def edit(self, path, old, new, count=1):
        files = dict(self.files)
        self.assertIn(old, files[path])
        files[path] = files[path].replace(old, new, count)
        return files

    def test_mapping_covers_every_item_exactly_once(self):
        items, _, _ = cut_items(self.commands)
        names = sorted({item.name() for item in items})
        mapped = sorted(name for group in COMMAND_CHILDREN.values() for name in group)
        self.assertEqual(names, mapped)
        self.assertEqual(len(items), 92)

    def test_honest_split_has_no_finding(self):
        self.assertEqual(self.findings(), [])

    def test_honest_split_with_the_declaration_has_no_finding(self):
        self.assertEqual(self.findings(lib=with_declaration(self.new_lib)), [])

    def test_every_file_is_within_the_limit(self):
        for path, text in self.files.items():
            with self.subTest(path=path):
                self.assertLessEqual(counted_lines(text), LIMIT)
        self.assertGreaterEqual(len(self.files), 13)

    def test_commands_rs_keeps_no_command(self):
        self.assertNotIn("#[tauri::command]", self.files[COMMANDS])

    def test_moved_member_deleted_fails(self):
        files = self.edit(self.child("transport"), "pub fn resume(", "fn resume_gone(")
        start = files[self.child("transport")].index("#[tauri::command]\nfn resume_gone(")
        end = files[self.child("transport")].index("\n}\n", start) + 3
        text = files[self.child("transport")]
        files[self.child("transport")] = text[:start] + text[end:]
        self.assertTrue(compare_sources({COMMANDS: self.commands}, files, "commands"))

    def test_child_mod_line_removed_fails(self):
        files = self.edit(COMMANDS, "pub(crate) mod radio;\n", "")
        self.assertTrue(any("not part of the build" in f for f in self.findings(files)))

    def test_tauri_command_removed_fails(self):
        files = self.edit(self.child("transport"), "#[tauri::command]\npub fn pause(", "pub fn pause(")
        self.assertTrue(compare_sources({COMMANDS: self.commands}, files, "commands"))
        self.assertTrue(commands_check.split_findings(self.names, files, self.new_lib))

    def test_command_renamed_in_fn_and_lib_fails(self):
        files = self.edit(self.child("transport"), "pub fn pause(", "pub fn pause_playback(")
        lib = self.new_lib.replace("commands::transport::pause,", "commands::transport::pause_playback,")
        self.assertTrue(compare_sources({COMMANDS: self.commands}, files, "commands"))
        self.assertTrue(moves.lib_findings(self.lib, lib))
        self.assertTrue(commands_check.split_findings(self.names, files, lib))

    def test_two_handler_entries_swapped_fails(self):
        lib = self.new_lib.replace(
            "commands::transport::pause,\n            commands::transport::resume,",
            "commands::transport::resume,\n            commands::transport::pause,",
        )
        self.assertNotEqual(lib, self.new_lib)
        self.assertTrue(moves.lib_findings(self.lib, lib))

    def test_handler_entry_with_another_last_segment_fails(self):
        lib = self.new_lib.replace("commands::transport::pause,", "commands::transport::resume,")
        self.assertTrue(moves.lib_findings(self.lib, lib))
        self.assertTrue(commands_check.split_findings(self.names, self.files, lib))

    def test_use_added_to_lib_fails(self):
        lib = self.new_lib.replace("use std::path::PathBuf;", "use std::path::PathBuf;\nuse std::fs;")
        self.assertTrue(moves.lib_findings(self.lib, lib))

    def test_glob_handler_entry_fails(self):
        lib = self.new_lib.replace("commands::queue::add_to_queue,", "commands::queue::*,")
        self.assertTrue(moves.lib_findings(self.lib, lib))

    def test_declaration_without_cfg_test_fails(self):
        lib = self.new_lib.replace("mod events;\n", "mod events;\nmod wire_format_tests;\n", 1)
        self.assertTrue(moves.lib_findings(self.lib, lib))

    def test_events_body_change_fails(self):
        changed = self.events.replace(
            "crate::commands::radio_mode_label(cq)", "crate::commands::radio_mode_label(cq).clone()"
        )
        self.assertNotEqual(changed, self.events)
        self.assertTrue(compare_sources({moves.EVENTS: self.events}, {moves.EVENTS: changed}))

    def test_events_use_change_passes(self):
        changed = self.events.replace("use std::sync::Arc;\n", "use std::sync::{Arc};\n", 1)
        self.assertEqual(compare_sources({moves.EVENTS: self.events}, {moves.EVENTS: changed}), [])

    def test_add_music_source_variants_in_different_children_fail(self):
        children = {k: list(v) for k, v in COMMAND_CHILDREN.items()}
        files, lib = build_commands_split(self.commands, self.lib, children)
        text = files[self.child("local_sources")]
        second = text.index("#[tauri::command]\n#[cfg(target_os = \"android\")]")
        end = text.index("\n}\n", second) + 3
        files[self.child("local_sources")] = text[:second] + text[end:]
        files[self.child("library")] += "\n" + text[second:end]
        self.assertEqual(compare_sources({COMMANDS: self.commands}, files, "commands"), [])
        found = commands_check.split_findings(self.names, files, lib)
        self.assertTrue(any("variants of add_music_source" in f for f in found), found)

    def test_handler_naming_the_wrong_child_fails(self):
        lib = self.new_lib.replace("commands::transport::pause,", "commands::radio::pause,")
        self.assertEqual(moves.lib_findings(self.lib, lib), [])
        self.assertTrue(commands_check.split_findings(self.names, self.files, lib))

    def test_handler_without_child_module_fails(self):
        lib = self.new_lib.replace("commands::transport::pause,", "commands::pause,")
        self.assertTrue(commands_check.split_findings(self.names, self.files, lib))

    def test_vague_child_name_fails(self):
        files = dict(self.files)
        files[self.child("helpers")] = files.pop(self.child("eq"))
        files[COMMANDS] = files[COMMANDS].replace("pub(crate) mod eq;", "pub(crate) mod helpers;")
        self.assertTrue(moves.vague_name_findings(files))

    def test_shadowing_child_name_fails(self):
        files = {f"{COMMANDS_DIR}/streaming.rs": "", f"{COMMANDS_DIR}/tauri/mod.rs": ""}
        self.assertEqual(len(moves.vague_name_findings(files)), 2)


class DtoSplit(Base, unittest.TestCase):
    def setUp(self):
        self.files = build_dto_split(self.dto)

    def findings(self, files=None):
        files = self.files if files is None else files
        return compare_sources({DTO: self.dto}, files, "dto") + moves.vague_name_findings(files)

    def child(self, name):
        return f"{DTO_DIR}/{name}.rs"

    def edit(self, path, old, new):
        files = dict(self.files)
        self.assertIn(old, files[path])
        files[path] = files[path].replace(old, new, 1)
        return files

    def test_mapping_covers_every_item(self):
        items, _, _ = cut_items(self.dto)
        names = {item.name() for item in items}
        mapped = {name for group in DTO_CHILDREN.values() for name in group}
        self.assertEqual(names - {"local_source_tests"}, mapped)

    def test_honest_split_has_no_finding(self):
        self.assertEqual(self.findings(), [])

    def test_every_file_is_within_the_limit(self):
        for path, text in self.files.items():
            with self.subTest(path=path):
                self.assertLessEqual(counted_lines(text), LIMIT)

    def test_dto_rs_defines_no_struct_or_enum_outside_its_tests(self):
        self.assertEqual(dto_check.dto_findings(self.files[DTO]), [])
        self.assertTrue(dto_check.dto_findings(self.dto))

    def test_cache_status_and_local_source_tests_stay_in_dto_rs(self):
        self.assertIn("pub use nexus_player_backend::library::types::CacheStatus;", self.files[DTO])
        self.assertIn("mod local_source_tests {", self.files[DTO])

    def test_serde_rename_all_changed_fails(self):
        files = self.edit(self.child("eq"), 'rename_all = "camelCase"', 'rename_all = "snake_case"')
        self.assertTrue(self.findings(files))

    def test_serde_tag_changed_fails(self):
        files = self.edit(self.child("queue"), 'tag = "entryType"', 'tag = "entry_type"')
        self.assertTrue(self.findings(files))

    def test_two_fields_swapped_fails(self):
        files = self.edit(
            self.child("eq"),
            "    pub name: String,\n    pub gains: Vec<f32>,",
            "    pub gains: Vec<f32>,\n    pub name: String,",
        )
        self.assertTrue(self.findings(files))

    def test_derive_removed_fails(self):
        files = self.edit(
            self.child("downloads"),
            "#[derive(Debug, Clone, Serialize, Deserialize)]",
            "#[derive(Debug, Clone, Serialize)]",
        )
        self.assertTrue(self.findings(files))

    def test_glob_pub_use_fails(self):
        files = self.edit(DTO, "pub use eq::{EqSettingsDto, EqPresetDto};", "pub use eq::*;")
        self.assertTrue(self.findings(files))

    def test_child_mod_line_removed_fails(self):
        files = self.edit(DTO, "mod eq;\n", "")
        self.assertTrue(self.findings(files))


if __name__ == "__main__":
    unittest.main()
