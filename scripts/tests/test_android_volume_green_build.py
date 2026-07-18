"""R6: the Android target compiles the cfg(target_os = "android") volume policy."""
import os
import pathlib
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
NDK = pathlib.Path(os.environ.get(
    "NDK_HOME", "/home/syndafloden/Android/Sdk/ndk/30.0.16248370"))
BIN = NDK / "toolchains/llvm/prebuilt/linux-x86_64/bin"


class GreenBuild(unittest.TestCase):
    # R6
    def test_android_cargo_check_passes(self):
        if not BIN.is_dir():
            self.skipTest("NDK not available")
        env = dict(os.environ,
                   CC_aarch64_linux_android=str(BIN / "aarch64-linux-android24-clang"),
                   CXX_aarch64_linux_android=str(BIN / "aarch64-linux-android24-clang++"),
                   AR_aarch64_linux_android=str(BIN / "llvm-ar"))
        r = subprocess.run(
            ["cargo", "check", "--target", "aarch64-linux-android",
             "-p", "nexus-player-frontend", "--locked", "--offline"],
            cwd=ROOT, env=env, capture_output=True, text=True)
        subprocess.run(["git", "checkout", "--", "nexus-player-frontend/src-tauri/gen/android"], cwd=ROOT)
        self.assertEqual(r.returncode, 0, r.stderr[-3000:])

    # R6
    def test_file_size_cap(self):
        r = subprocess.run(["python3", "scripts/check_file_sizes.py"], cwd=ROOT,
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)


if __name__ == "__main__":
    unittest.main()
