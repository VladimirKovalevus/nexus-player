"""R5: volume UI is gated by the shared isAndroid helper on Android."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
COMP = ROOT / "nexus-player-frontend/src/components"
PLAYER = COMP / "PlayerControl.vue"
OVERLAY = COMP / "VisualizerOverlay.vue"


def open_tag_containing(src: str, needle_re: str) -> str:
    """Return the full opening tag (attributes included) that matches needle_re."""
    for m in re.finditer(r"<[A-Za-z][^<>]*?>", src, re.S):
        if re.search(needle_re, m.group(0)):
            return m.group(0)
    return ""


def excludes_android(tag: str) -> bool:
    m = re.search(r'v-if\s*=\s*"([^"]*)"', tag)
    return bool(m) and bool(re.search(r"!\s*isAndroid\b|isAndroid\s*===?\s*false", m.group(1)))


class AndroidVolumeUi(unittest.TestCase):
    def test_r5_components_import_helper(self):
        for f in (PLAYER, OVERLAY):
            self.assertRegex(
                f.read_text(),
                r"import\s*\{[^}]*\bisAndroid\b[^}]*\}\s*from\s*['\"][^'\"]*utils/platform(\.ts)?['\"]",
                f.name,
            )

    def test_r5_player_volume_wrapper_gated(self):
        tag = open_tag_containing(PLAYER.read_text(), r'@wheel[^=\s]*\s*=\s*"handleVolumeWheel"')
        self.assertTrue(tag, "volume wrapper with wheel handler not found")
        self.assertTrue(excludes_android(tag), tag)

    def test_r5_overlay_volume_section_gated(self):
        tag = open_tag_containing(OVERLAY.read_text(), r'class\s*=\s*"[^"]*\bvolume-section\b')
        self.assertTrue(tag, "volume-section element not found")
        self.assertTrue(excludes_android(tag), tag)


if __name__ == "__main__":
    unittest.main()
