"""T3 R2, R3, R4, R5, R16: self-tests for the T3 moves check, lib.rs rule, tree guard, command
extraction, dto shape and test-name rule, on in-memory fixtures and git objects only."""

import os
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))

from moves_check import compare_sources  # noqa: E402
import t3_commands_check as commands_check  # noqa: E402
import t3_dto_check as dto_check  # noqa: E402
import t3_gitleaks_check as gitleaks_check  # noqa: E402
import t3_moves_check as moves  # noqa: E402
import t3_test_names as test_names  # noqa: E402

ROOT = "src/commands.rs"
DTO_ROOT = "src/dto.rs"


def base_sha():
    return (REPO / moves.BASE_FILE).read_text(encoding="utf-8").strip()


def git_show(path):
    return subprocess.run(
        ["git", "show", f"{base_sha()}:{path}"], cwd=REPO, capture_output=True, check=True
    ).stdout.decode("utf-8")


COMMANDS_BASE = """use crate::dto::TrackDto;
use tauri::State;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
#[cfg(not(target_os = "android"))]
pub async fn add_music_source(state: State<'_, X>) -> Result<Option<i64>> {
    Ok(None)
}

#[tauri::command]
#[cfg(target_os = "android")]
pub async fn add_music_source(state: State<'_, X>) -> Result<Option<i64>> {
    Err("android".into())
}

// The parameter list is the invoke contract with the frontend, so it cannot be grouped.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create(a: i64, b: i64) -> Result<i64> {
    Ok(a + b)
}

pub(crate) fn radio_mode_label(x: &str) -> Option<String> {
    Some(x.to_string())
}

use nexus_player_backend::config::AppConfig;

#[tauri::command]
pub fn get_app_config() -> Result<AppConfig> {
    Ok(AppConfig::default())
}
"""

SOURCES_CHILD = """use super::Result;
use crate::dto::TrackDto;
use tauri::State;

#[tauri::command]
#[cfg(not(target_os = "android"))]
pub async fn add_music_source(state: State<'_, X>) -> Result<Option<i64>> {
    Ok(None)
}

#[tauri::command]
#[cfg(target_os = "android")]
pub async fn add_music_source(state: State<'_, X>) -> Result<Option<i64>> {
    Err("android".into())
}
"""

PLAYLISTS_CHILD = """use super::Result;

// The parameter list is the invoke contract with the frontend, so it cannot be grouped.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create(a: i64, b: i64) -> Result<i64> {
    Ok(a + b)
}
"""

RADIO_CHILD = """pub(crate) fn radio_mode_label(x: &str) -> Option<String> {
    Some(x.to_string())
}
"""

CONFIG_CHILD = """use super::Result;
use nexus_player_backend::config::AppConfig;

#[tauri::command]
pub fn get_app_config() -> Result<AppConfig> {
    Ok(AppConfig::default())
}
"""

COMMANDS_ROOT = """use crate::dto::TrackDto;

pub(crate) mod app_config;
pub(crate) mod local_sources;
pub(crate) mod playlists;
pub(crate) mod radio;

pub(crate) use radio::radio_mode_label;

type Result<T> = std::result::Result<T, String>;
"""

DTO_BASE = """use nexus_player_backend::audio::types::TrackState;
use serde::{Deserialize, Serialize};

pub use nexus_player_backend::library::types::CacheStatus;
use nexus_player_backend::streaming::types as streaming;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub id: i64,
}

use nexus_player_backend::audio::types::{self as audio};
use nexus_player_backend::library::models;

impl From<audio::Track> for TrackDto {
    fn from(track: audio::Track) -> Self {
        Self { id: hash_str_to_i64("x") }
    }
}

pub(crate) fn hash_str_to_i64(s: &str) -> i64 {
    0
}

#[cfg(test)]
mod local_source_tests {
    use super::*;

    #[test]
    fn t() {}
}
"""

DTO_ROOT_SPLIT = """use serde::{Deserialize, Serialize};

pub use nexus_player_backend::library::types::CacheStatus;

mod track;

pub use track::TrackDto;
pub(crate) use track::hash_str_to_i64;

#[cfg(test)]
mod local_source_tests {
    use super::*;
    use super::track::TrackDto as _;

    #[test]
    fn t() {}
}
"""

DTO_TRACK = """use super::*;
use nexus_player_backend::streaming::types as streaming;
use nexus_player_backend::audio::types::{self as audio};
use nexus_player_backend::library::models;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub id: i64,
}

impl From<audio::Track> for TrackDto {
    fn from(track: audio::Track) -> Self {
        Self { id: hash_str_to_i64("x") }
    }
}

pub(crate) fn hash_str_to_i64(s: &str) -> i64 {
    0
}
"""


def commands_split():
    return {
        ROOT: COMMANDS_ROOT,
        "src/commands/local_sources.rs": SOURCES_CHILD,
        "src/commands/playlists.rs": PLAYLISTS_CHILD,
        "src/commands/radio.rs": RADIO_CHILD,
        "src/commands/app_config.rs": CONFIG_CHILD,
    }


class CompareSourcesAcceptsT3Constructs(unittest.TestCase):
    """R2: what commands.rs and dto.rs need that T2's files did not have."""

    def commands(self, files):
        return compare_sources({ROOT: COMMANDS_BASE}, files, "commands")

    def test_honest_commands_split(self):
        self.assertEqual(self.commands(commands_split()), [])

    def test_pub_crate_mod_declarations(self):
        files = commands_split()
        self.assertIn("pub(crate) mod radio;", files[ROOT])
        self.assertEqual(self.commands(files), [])

    def test_plain_mod_declaration(self):
        files = commands_split()
        files[ROOT] = files[ROOT].replace("pub(crate) mod radio;", "mod radio;")
        self.assertEqual(self.commands(files), [])

    def test_pub_crate_use_reexport_is_dropped(self):
        files = commands_split()
        files[ROOT] = files[ROOT].replace("pub(crate) use radio::radio_mode_label;\n", "")
        self.assertEqual(self.commands(files), [])

    def test_mid_file_use_moved_to_a_child(self):
        self.assertIn("use nexus_player_backend::config::AppConfig;", CONFIG_CHILD)
        self.assertEqual(self.commands(commands_split()), [])

    def test_attribute_on_mod_declaration_fails(self):
        files = commands_split()
        files[ROOT] = files[ROOT].replace("pub(crate) mod radio;", "#[cfg(test)]\npub(crate) mod radio;")
        self.assertTrue(self.commands(files))

    def test_pub_mod_declaration_fails(self):
        files = commands_split()
        files[ROOT] = files[ROOT].replace("pub(crate) mod radio;", "pub mod radio;")
        self.assertTrue(self.commands(files))

    def test_cfg_variant_swapped_fails(self):
        files = commands_split()
        child = "src/commands/local_sources.rs"
        files[child] = files[child].replace('#[cfg(target_os = "android")]', "#[cfg(unix)]")
        self.assertTrue(self.commands(files))

    def test_allow_dropped_fails(self):
        files = commands_split()
        child = "src/commands/playlists.rs"
        files[child] = files[child].replace("#[allow(clippy::too_many_arguments)]\n", "")
        self.assertTrue(self.commands(files))

    def test_command_attribute_dropped_fails(self):
        files = commands_split()
        child = "src/commands/app_config.rs"
        files[child] = files[child].replace("#[tauri::command]\n", "")
        self.assertTrue(self.commands(files))

    def test_pub_widened_from_pub_crate_is_not_a_move(self):
        files = commands_split()
        child = "src/commands/radio.rs"
        files[child] = files[child].replace("pub(crate) fn", "pub fn")
        self.assertTrue(self.commands(files))

    def dto(self, files):
        return compare_sources({DTO_ROOT: DTO_BASE}, files, "dto")

    def test_honest_dto_split_with_aliases_and_cache_status(self):
        files = {DTO_ROOT: DTO_ROOT_SPLIT, "src/dto/track.rs": DTO_TRACK}
        self.assertEqual(self.dto(files), [])

    def test_cache_status_reexport_is_an_ordinary_segment(self):
        root = DTO_ROOT_SPLIT.replace("pub use nexus_player_backend::library::types::CacheStatus;\n", "")
        self.assertTrue(self.dto({DTO_ROOT: root, "src/dto/track.rs": DTO_TRACK}))

    def test_glob_reexport_fails(self):
        root = DTO_ROOT_SPLIT.replace("pub use track::TrackDto;", "pub use track::*;")
        self.assertTrue(self.dto({DTO_ROOT: root, "src/dto/track.rs": DTO_TRACK}))

    def test_reexport_of_undeclared_child_fails(self):
        root = DTO_ROOT_SPLIT.replace("pub use track::TrackDto;", "pub use other::TrackDto;")
        self.assertTrue(self.dto({DTO_ROOT: root, "src/dto/track.rs": DTO_TRACK}))


LIB_BASE = """mod commands;
mod dto;
mod events;

use std::sync::Arc;

pub fn run() {
    tauri::Builder::default()
        // A comment.
        .invoke_handler(tauri::generate_handler![
            commands::get_all_tracks,
            commands::pause,
            commands::resume,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
"""

LIB_SPLIT = LIB_BASE.replace("commands::get_all_tracks,", "commands::library::get_all_tracks,").replace(
    "commands::pause,", "commands::transport::pause,"
).replace("commands::resume,", "commands::transport::playback::resume")


class LibRule(unittest.TestCase):
    """R3: lib.rs changes only by handler path prefixes and the wire-format declaration."""

    def test_unchanged(self):
        self.assertEqual(moves.lib_findings(LIB_BASE, LIB_BASE), [])

    def test_child_paths_and_no_trailing_comma(self):
        self.assertEqual(moves.lib_findings(LIB_BASE, LIB_SPLIT), [])

    def test_declaration_anywhere_at_module_level(self):
        for text in (
            LIB_SPLIT.replace("mod events;\n", "mod events;\n#[cfg(test)]\nmod wire_format_tests;\n"),
            "#[cfg(test)] mod wire_format_tests;\n" + LIB_SPLIT,
            LIB_SPLIT + "\n#[cfg(test)]\nmod wire_format_tests;\n",
        ):
            with self.subTest(text=text[:60]):
                self.assertEqual(moves.lib_findings(LIB_BASE, text), [])

    def test_declaration_twice_fails(self):
        text = LIB_SPLIT + "\n#[cfg(test)]\nmod wire_format_tests;\n" * 2
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_declaration_inside_a_function_fails(self):
        text = LIB_SPLIT.replace("pub fn run() {\n", "pub fn run() {\n    #[cfg(test)]\n    mod wire_format_tests;\n")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_declaration_without_cfg_test_fails(self):
        text = LIB_SPLIT.replace("mod events;\n", "mod events;\nmod wire_format_tests;\n")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_other_module_declared_fails(self):
        text = LIB_SPLIT.replace("mod events;\n", "mod events;\n#[cfg(test)]\nmod other_tests;\n")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_new_use_fails(self):
        text = LIB_SPLIT.replace("use std::sync::Arc;", "use std::sync::Arc;\nuse commands::transport::pause;")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_reordered_entries_fail(self):
        text = LIB_SPLIT.replace(
            "commands::transport::pause,\n            commands::transport::playback::resume",
            "commands::transport::playback::resume,\n            commands::transport::pause",
        )
        self.assertNotEqual(text, LIB_SPLIT)
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_glob_entry_fails(self):
        text = LIB_SPLIT.replace("commands::transport::pause,", "commands::transport::*,")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_crate_prefixed_entry_fails(self):
        text = LIB_SPLIT.replace("commands::transport::pause,", "crate::commands::transport::pause,")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_changed_last_segment_fails(self):
        text = LIB_SPLIT.replace("commands::transport::pause,", "commands::transport::stop,")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_removed_entry_fails(self):
        text = LIB_SPLIT.replace("commands::transport::pause,\n", "")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_comment_changed_fails(self):
        text = LIB_SPLIT.replace("// A comment.", "// Another comment.")
        self.assertTrue(moves.lib_findings(LIB_BASE, text))

    def test_string_changed_outside_the_handler_fails(self):
        text = LIB_SPLIT.replace("error while running", "error running")
        found = moves.lib_findings(LIB_BASE, text)
        self.assertTrue(found)
        self.assertIn(":16:", found[0])

    def test_real_base_lib_passes_with_and_without_declaration(self):
        base = git_show(moves.LIB)
        self.assertEqual(moves.lib_findings(base, base), [])
        declared = base.replace("mod events;\n", "mod events;\n#[cfg(test)]\nmod wire_format_tests;\n", 1)
        self.assertEqual(moves.lib_findings(base, declared), [])
        self.assertEqual(moves.declaration_findings(declared), [])
        self.assertTrue(moves.declaration_findings(base))


class TreeGuard(unittest.TestCase):
    """R16, R2, R8, R13: what T3 may touch, and the content rules for licences and manifests."""

    LICENSE = "MIT License\n\nCopyright (c) 2026 Syndafloden\n\nPermission is hereby granted.\n"
    MANIFEST = '[package]\nname = "x"\nversion = "0.1.0"\ndescription = "A Tauri App"\nauthors = ["you"]\n\n[dependencies]\nserde = "1"\n'
    PACKAGE = '{\n  "name": "nexus-player-frontend",\n  "private": true\n}\n'

    def setUp(self):
        self.base = {
            "LICENSE": self.LICENSE,
            "crates/streaming/LICENSE": self.LICENSE,
            "nexus-player-backend/Cargo.toml": self.MANIFEST,
            "nexus-player-frontend/src-tauri/Cargo.toml": self.MANIFEST,
            "crates/streaming/Cargo.toml": self.MANIFEST,
            "nexus-player-frontend/package.json": self.PACKAGE,
            "types.md": "x\n",
            "CONTRIBUTING.md": "x\n",
        }
        self.current = dict(self.base)
        self.locked = {"scripts/moves_check.py", "scripts/check_file_sizes.py",
                       "scripts/tests/test_check_file_sizes.py", "scripts/acceptance/fmt.sh"}

    def findings(self, paths):
        return moves.guard_findings(paths, self.locked, self.base.get, self.current.get)

    def test_allowed_paths(self):
        paths = [
            "nexus-player-frontend/src-tauri/src/commands.rs",
            "nexus-player-frontend/src-tauri/src/commands/queue.rs",
            "nexus-player-frontend/src-tauri/src/commands/queue/state.rs",
            "nexus-player-frontend/src-tauri/src/dto.rs",
            "nexus-player-frontend/src-tauri/src/dto/track.rs",
            moves.LIB, moves.EVENTS, moves.WIRE_TESTS,
            "scripts/t3_new.py", "scripts/check_file_sizes.py",
            "scripts/tests/test_check_file_sizes.py",
            "README.md", "THIRD_PARTY_NOTICES.md", "crates/streaming/README.md",
            ".github/workflows/ci.yml", "vendor/butterchurn/raw.md",
        ]
        self.assertEqual(self.findings(paths), [])

    def test_forbidden_paths(self):
        for path in (
            "Cargo.lock", "Cargo.toml", "nexus-player-frontend/src-tauri/tauri.conf.json",
            "nexus-player-backend/src/db/migrations/m1.sql", "CLAUDE.md", ".gitattributes",
            ".gitignore", "nexus-player-frontend/.gitignore", ".cargo/config.toml", "clippy.toml",
            ".clippy.toml", "rustfmt.toml", "rust-toolchain", "rust-toolchain.toml",
            ".gitleaks.toml", ".gitleaksignore", "logo.png",
            "nexus-player-frontend/src-tauri/src/main.rs", "nexus-player-frontend/src-tauri/build.rs",
            "nexus-player-frontend/src-tauri/src/other.rs", "nexus-player-frontend/src/App.vue",
            "nexus-player-frontend/src-tauri/src/commands/notes.md",
            "vendor/butterchurn/src/visualizer.js", "nexus-player-backend/src/lib.rs",
            "crates/streaming/src/lib.rs", ".github/workflows/other.yml",
            "scripts/moves_check.py", "scripts/acceptance/fmt.sh",
        ):
            with self.subTest(path=path):
                self.assertTrue(self.findings([path]))

    def test_license_copyright_line_only(self):
        self.current["LICENSE"] = self.LICENSE.replace("Syndafloden", "Vladimir Kovalev")
        self.assertEqual(self.findings(["LICENSE"]), [])
        self.current["LICENSE"] += "extra\n"
        self.assertTrue(self.findings(["LICENSE"]))

    def test_license_deleted_fails(self):
        self.current["LICENSE"] = None
        self.assertTrue(self.findings(["LICENSE"]))

    def test_documents_only_deleted(self):
        self.current["types.md"] = None
        self.current["CONTRIBUTING.md"] = None
        self.assertEqual(self.findings(["types.md", "CONTRIBUTING.md"]), [])
        self.current["types.md"] = "changed\n"
        self.assertTrue(self.findings(["types.md"]))

    def test_manifest_package_keys(self):
        path = "nexus-player-frontend/src-tauri/Cargo.toml"
        self.current[path] = self.MANIFEST.replace(
            'description = "A Tauri App"\nauthors = ["you"]',
            'description = "Desktop music player"\nauthors = ["A <a@b.c>"]\nlicense = "MIT"\nrepository = "https://x"',
        )
        self.assertEqual(self.findings([path]), [])
        self.current[path] += "\n[features]\nx = []\n"
        self.assertTrue(self.findings([path]))

    def test_description_only_in_src_tauri(self):
        path = "nexus-player-backend/Cargo.toml"
        self.current[path] = self.MANIFEST.replace("A Tauri App", "Backend")
        self.assertTrue(self.findings([path]))

    def test_manifest_dependency_change_fails(self):
        path = "crates/streaming/Cargo.toml"
        self.current[path] = self.MANIFEST.replace('serde = "1"', 'serde = "1.0"')
        self.assertTrue(self.findings([path]))

    def test_package_json_license_only(self):
        path = "nexus-player-frontend/package.json"
        self.current[path] = '{"name": "nexus-player-frontend", "private": true, "license": "MIT"}'
        self.assertEqual(self.findings([path]), [])
        self.current[path] = '{"name": "nexus-player-frontend", "private": false, "license": "MIT"}'
        self.assertTrue(self.findings([path]))

    def test_unmodified_tree_has_no_finding(self):
        self.assertEqual(self.findings([]), [])


class VagueNames(unittest.TestCase):
    def test_vague_and_shadowing_names(self):
        for name in ("part", "part2", "misc", "util", "utils", "helper", "helpers", "common",
                     "other", "extra", "rest", "impl", "impl3", "streaming", "tauri", "std",
                     "serde", "nexus_player_backend"):
            for path in (f"{moves.SRC}/commands/{name}.rs", f"{moves.SRC}/dto/{name}/mod.rs"):
                with self.subTest(path=path):
                    self.assertTrue(moves.vague_name_findings([path]))

    def test_responsible_names_pass(self):
        paths = [f"{moves.SRC}/commands/queue.rs", f"{moves.SRC}/dto/track/mod.rs",
                 f"{moves.SRC}/commands/partition.rs", f"{moves.SRC}/commands.rs"]
        self.assertEqual(moves.vague_name_findings(paths), [])


class CommandExtraction(unittest.TestCase):
    """R3: names come from the code, never from a hand-written list."""

    def test_base_has_88_names_and_the_handler_matches(self):
        names, problems = commands_check.base_findings(
            git_show(commands_check.COMMANDS), git_show(moves.LIB)
        )
        self.assertEqual(problems, [])
        self.assertEqual(len(names), 88)
        self.assertEqual(names["add_music_source"], 2)
        self.assertIn("radio_mode_label", git_show(commands_check.COMMANDS))
        self.assertNotIn("radio_mode_label", names)
        self.assertNotIn("build_queue_state_dto", names)

    def test_attributes_and_docs_between_attribute_and_fn(self):
        text = (
            "#[tauri::command]\n/// Docs.\n#[cfg(unix)]\n// plain\npub async fn a() {}\n"
            '// #[tauri::command]\npub fn b() {}\nconst S: &str = "#[tauri::command] pub fn c";\n'
        )
        found, problems = commands_check.command_fns("x.rs", text)
        self.assertEqual([name for name, _ in found], ["a"])
        self.assertEqual(problems, [])

    def test_command_not_followed_by_pub_fn_is_a_finding(self):
        _, problems = commands_check.command_fns("x.rs", "#[tauri::command]\nfn a() {}\n")
        self.assertTrue(problems)

    def test_invoke_literals(self):
        text = (
            "invoke('a_b'); invoke<LibraryPageDto>(\n  'c1', {});\n"
            'invoke<Array<X>>("d"); invoke(command); rawInvoke(`e`); invoke(`f`);\n'
        )
        self.assertEqual(commands_check.invoke_literals({"x.ts": text}), {"a_b", "c1", "d", "f"})

    def test_invoke_rule(self):
        names = {"a": 1, "b": 1}
        self.assertEqual(commands_check.invoke_findings(names, {"x": "invoke('a')"}, {}), [])
        self.assertTrue(commands_check.invoke_findings(names, {"x": "invoke('z')"}, {}))
        self.assertEqual(
            commands_check.invoke_findings(names, {"x": "invoke('z')"}, {"x": "invoke('z')"}), []
        )

    def test_real_frontend_literals_are_commands(self):
        root = REPO / commands_check.FRONTEND
        texts = {str(p): p.read_text(encoding="utf-8") for p in root.rglob("*")
                 if p.suffix in (".ts", ".vue") and p.is_file()}
        names, _ = commands_check.base_findings(
            git_show(commands_check.COMMANDS), git_show(moves.LIB)
        )
        literals = commands_check.invoke_literals(texts)
        self.assertGreater(len(literals), 50)
        self.assertEqual(literals - set(names), set())


class DtoShape(unittest.TestCase):
    def test_base_dto_defines_structs(self):
        self.assertTrue(dto_check.dto_findings(git_show(moves.FAMILIES["dto"])))

    def test_split_root_passes(self):
        self.assertEqual(dto_check.dto_findings(DTO_ROOT_SPLIT), [])

    def test_test_module_moved_out_fails(self):
        root = DTO_ROOT_SPLIT.split("#[cfg(test)]")[0] + "#[cfg(test)]\nmod local_source_tests;\n"
        self.assertTrue(dto_check.dto_findings(root))

    def test_struct_in_comment_or_test_module_is_fine(self):
        root = DTO_ROOT_SPLIT.replace("    #[test]", "    struct Fixture;\n    #[test]") + "// pub struct X\n"
        self.assertEqual(dto_check.dto_findings(root), [])

    def test_enum_left_fails(self):
        self.assertTrue(dto_check.dto_findings(DTO_ROOT_SPLIT + "pub enum E { A }\n"))


class TestNames(unittest.TestCase):
    """R5: the baseline plus wire-format tests, in both directions."""

    BASELINE = {"dto::local_source_tests::missing_source_is_rejected", "a::b"}

    def wire(self, count=13):
        return {f"wire_format_tests::{prefix}_x" for prefix in test_names.PREFIXES[:count]}

    def test_listing_parse(self):
        text = "a::b: test\nc: bench\nwire_format_tests::x: test\n\n1 test, 0 benchmarks\n"
        self.assertEqual(test_names.names_from_listing(text), {"a::b", "wire_format_tests::x"})

    def test_baseline_plus_wire_passes(self):
        self.assertEqual(test_names.test_name_findings(self.BASELINE, self.BASELINE | self.wire(), True), [])

    def test_without_wire_passes_without_minimum(self):
        self.assertEqual(test_names.test_name_findings(self.BASELINE, set(self.BASELINE), False), [])
        self.assertTrue(test_names.test_name_findings(self.BASELINE, set(self.BASELINE), True))

    def test_moved_test_path_fails(self):
        names = {"dto::track::local_source_tests::missing_source_is_rejected", "a::b"} | self.wire()
        self.assertTrue(test_names.test_name_findings(self.BASELINE, names, False))

    def test_foreign_new_test_fails(self):
        names = self.BASELINE | self.wire() | {"commands::queue::tests::x"}
        self.assertTrue(test_names.test_name_findings(self.BASELINE, names, False))

    def test_missing_prefix_fails(self):
        names = self.BASELINE | self.wire(12) | {f"wire_format_tests::extra_{i}" for i in range(5)}
        self.assertTrue(test_names.test_name_findings(self.BASELINE, names, True))

    def test_real_baseline_has_the_local_source_tests(self):
        baseline = set((REPO / test_names.BASELINE).read_text(encoding="utf-8").split())
        self.assertGreaterEqual(len(baseline), 120)
        self.assertIn("dto::local_source_tests::missing_source_is_rejected", baseline)


class GitleaksSelection(unittest.TestCase):
    """R15: untracked zero-byte placeholders are skipped, tracked empty files are kept."""

    def test_selection(self):
        sizes = {"README.md": 10, "empty_tracked": 0, ".gitleaks.toml": 0, "new.md": 5, "gone": None}

        def lstat(path):
            size = sizes[path]
            return None if size is None else os.stat_result((0o100644, 0, 0, 1, 0, 0, size, 0, 0, 0))

        copy, skipped = gitleaks_check.select_files(
            list(sizes), {"README.md", "empty_tracked", "gone"}, lstat
        )
        self.assertEqual(copy, ["README.md", "empty_tracked", "new.md"])
        self.assertEqual(skipped, [".gitleaks.toml"])

    def test_control_token_shape(self):
        token = gitleaks_check.control_token()
        self.assertRegex(token, r"^ghp_[A-Za-z0-9]{36}$")
        self.assertNotEqual(token, gitleaks_check.control_token())


class BaseFile(unittest.TestCase):
    def test_base_sha_is_usable(self):
        self.assertEqual(moves.base_sha_problems((REPO / moves.BASE_FILE).read_text()), [])

    def test_bad_base_sha(self):
        self.assertTrue(moves.base_sha_problems("not a sha"))


if __name__ == "__main__":
    unittest.main()
