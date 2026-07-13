"""R4: a single shared isAndroid helper in src/utils/platform.ts."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "nexus-player-frontend/src"
PLATFORM = SRC / "utils/platform.ts"
UA_REGEX = re.compile(r"/Android/[a-z]*\s*\.test\(\s*navigator\.userAgent|navigator\.userAgent\.match\(\s*/Android")


class AndroidVolumePlatform(unittest.TestCase):
    def test_r4_helper_exports_is_android(self):
        self.assertTrue(PLATFORM.exists(), "src/utils/platform.ts missing")
        src = PLATFORM.read_text()
        self.assertRegex(src, r"export\s+const\s+isAndroid\b")
        self.assertRegex(src, r"/Android/i?\s*\.test\(\s*navigator\.userAgent\s*\)")

    def test_r4_no_other_android_ua_regex(self):
        offenders = []
        for p in list(SRC.rglob("*.ts")) + list(SRC.rglob("*.vue")):
            if p == PLATFORM:
                continue
            if UA_REGEX.search(p.read_text()):
                offenders.append(str(p.relative_to(ROOT)))
        self.assertEqual(offenders, [])

    def _assert_imports(self, rel):
        src = (SRC / rel).read_text()
        self.assertRegex(
            src,
            r"import\s*\{[^}]*\bisAndroid\b[^}]*\}\s*from\s*['\"](?:@/|(?:\.\./|\./)+)(?:utils/)?platform['\"]",
        )

    def test_r4_library_view_uses_helper(self):
        self._assert_imports("views/LibraryView.vue")

    def test_r4_provider_auth_uses_helper(self):
        self._assert_imports("utils/providerAuth.ts")


if __name__ == "__main__":
    unittest.main()
