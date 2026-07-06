"""R3: MainActivity.onCreate pins the volume control stream to STREAM_MUSIC."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN = (
    ROOT
    / "nexus-player-frontend/src-tauri/gen/android/app/src/main/java"
    / "com/nexus_player_frontend/app/MainActivity.kt"
)


def on_create_body(src: str) -> str:
    m = re.search(r"override\s+fun\s+onCreate\s*\([^)]*\)\s*\{", src)
    if not m:
        return ""
    depth, i = 1, m.end()
    while i < len(src) and depth:
        depth += {"{": 1, "}": -1}.get(src[i], 0)
        i += 1
    return src[m.end():i]


class AndroidVolumeActivity(unittest.TestCase):
    def setUp(self):
        self.src = MAIN.read_text()

    def test_r3_imports_audio_manager(self):
        self.assertRegex(self.src, r"(?m)^import\s+android\.media\.AudioManager\s*$")

    def test_r3_on_create_sets_volume_stream(self):
        body = on_create_body(self.src)
        pattern = (
            r"volumeControlStream\s*=\s*AudioManager\.STREAM_MUSIC"
            r"|setVolumeControlStream\(\s*AudioManager\.STREAM_MUSIC\s*\)"
        )
        self.assertRegex(body, pattern)


if __name__ == "__main__":
    unittest.main()
