"""R6: build hygiene checks. The Android cargo check is manual (it cannot run in the sandbox)."""
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class GreenBuild(unittest.TestCase):
    # R6
    def test_file_size_cap(self):
        r = subprocess.run(["python3", "scripts/check_file_sizes.py"], cwd=ROOT,
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)


if __name__ == "__main__":
    unittest.main()
