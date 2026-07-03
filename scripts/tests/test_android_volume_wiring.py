"""Wiring checks for the Android volume policy (R1, R2), by reading sources."""
import re
import unittest
from pathlib import Path

AUDIO = Path(__file__).resolve().parents[2] / "nexus-player-backend" / "src" / "audio"
LITERAL = re.compile(r"\b0?\.10*(_?f32)?\b")
DIVISION = re.compile(r"1\.0(_?f32)?\s*/\s*10(\.0)?")


def read(name):
    return (AUDIO / name).read_text()


def body_from(text, start):
    """Return text from `start` through the matching closing brace."""
    open_idx = text.index("{", start)
    depth = 0
    for i in range(open_idx, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[start : i + 1]
    raise AssertionError("unbalanced braces")


def fn_body(text, signature_re, after=0):
    m = re.compile(signature_re).search(text, after)
    assert m, f"not found: {signature_re}"
    return body_from(text, m.start())


class AndroidVolumeWiring(unittest.TestCase):
    def setUp(self):
        self.playback = read("playback.rs")
        self.types = read("types.rs")
        self.engine = read("engine.rs")

    def new_body(self):
        impl = self.playback.index("impl PlaybackEngine")
        return fn_body(self.playback, r"fn new\s*\(", impl)

    def default_body(self):
        return fn_body(self.types, r"impl Default for AudioState")

    def test_set_volume_uses_effective_volume(self):  # R2
        body = fn_body(self.playback, r"fn set_volume\s*\(")
        self.assertIn("effective_volume(", body)

    def test_engine_new_uses_default_volume(self):  # R1
        self.assertIn("default_volume()", self.new_body())

    def test_audio_state_default_uses_default_volume(self):  # R1
        self.assertIn("default_volume()", self.default_body())

    def test_engine_set_volume_arm_reports_effective_value(self):  # R2
        m = re.search(r"AudioCommand::SetVolume\(\s*(\w+)\s*\)\s*=>", self.engine)
        self.assertIsNotNone(m, "SetVolume arm not found")
        binding = m.group(1)
        arm = body_from(self.engine, m.start())
        self.assertTrue(
            "effective_volume(" in arm or "playback.volume()" in arm,
            "arm must use the effective value",
        )
        self.assertNotRegex(arm, rf"\.volume\s*=\s*{binding}\s*;")
        self.assertNotRegex(arm, r"VolumeChanged\s*\{\s*" + binding + r"\s*\}")
        self.assertNotRegex(arm, rf"VolumeChanged\s*\{{\s*volume:\s*{binding}\s*\}}")

    def test_no_hardcoded_default_volume_literal(self):  # R1
        for name, text, scoped in (
            ("playback.rs", self.playback, self.new_body()),
            ("types.rs", self.types, self.default_body()),
        ):
            for line in text.splitlines():
                if "volume" in line.lower():
                    self.assertIsNone(LITERAL.search(line), f"{name}: {line.strip()}")
                    self.assertIsNone(DIVISION.search(line), f"{name}: {line.strip()}")
            self.assertIsNone(LITERAL.search(scoped), f"{name}: literal in body")
            self.assertIsNone(DIVISION.search(scoped), f"{name}: division in body")


if __name__ == "__main__":
    unittest.main()
