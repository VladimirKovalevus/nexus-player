"""Compliant T3 documents used by test_t3_docs.py. Together they pass every R7-R14 rule, which
proves the rules can hold at once. Fixtures hold no secret-like strings, because R15 scans
scripts/; the CI values here are placeholders, the real ones come from target/t3-ci/."""

RUST = "1.98.0"
GITLEAKS = "8.30.1"
DIGEST = "ab" * 32
CHECKSUMS = f"{DIGEST}  gitleaks_{GITLEAKS}_linux_x64.tar.gz\n"
CHECKOUT = "0123456789abcdef0123456789abcdef01234567"

README = """<p align="center"><img src="logo.png" alt="Nexus Player logo" width="160"></p>

# Nexus Player

A desktop and mobile music player built with Tauri 2, Vue 3 and a Rust audio engine. It plays a
local library and streams from SoundCloud and Bandcamp through one queue.

## Architecture

```mermaid
flowchart LR
    UI[Vue frontend] -->|invoke| Commands[Tauri commands]
    Commands --> Backend[nexus-player-backend]
    Backend --> Streaming[crates/streaming]
    Backend --> Decoder[Symphonia]
    Backend -->|raw samples| Visualizer[butterchurn]
```

## Engineering notes

- **Cursor queue**: the queue resolves nested playlists lazily through a cursor
  ([nexus-player-backend/src/playlist/cursor](nexus-player-backend/src/playlist/cursor)).
- **HLS and chunked streaming**: [hls_decoder.rs](nexus-player-backend/src/audio/hls_decoder.rs),
  [chunked.rs](nexus-player-backend/src/audio/chunked.rs) and [the HLS buffers](crates/streaming/src/hls).
- **Download manager**: [manager.rs](crates/streaming/src/state/manager.rs).
- **Artwork cache**: [crates/streaming/src/images](crates/streaming/src/images/).
- **Telegram sync**: [sync/telegram](nexus-player-backend/src/sync/telegram).
- **Visualizer**: [butterchurn](vendor/butterchurn) fed raw samples from the Rust backend.

## Features

- Local library with folder scanning, playlists and nested playlists.
- SoundCloud and Bandcamp search and playback.
- Queue with shuffle, repeat and radio mode; ten-band equalizer.

## Platforms

Built and run by the author on Linux, Windows and Android. macOS is not built or tested.

## Building

The build needs a Rust toolchain with cargo, Node.js with npm, and on Linux these packages:

```sh
sudo apt-get install clang mold cmake pkg-config libwebkit2gtk-4.1-dev \\
  libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libasound2-dev
```

Then run `npm install` and `npm run tauri build` in `nexus-player-frontend`.

## Repository layout

- `nexus-player-backend`: the Rust library with the audio engine and the library database.
- `nexus-player-frontend`: the Tauri app and its Vue interface.
- `crates/streaming`: buffering, HLS and downloads.
- `vendor`: patched third-party code, see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Disclaimer

The SoundCloud and Bandcamp integrations use unofficial, undocumented interfaces. This project
is not affiliated with those services, distributes no content, and is meant for educational and
personal use.

## License

MIT, see [LICENSE](LICENSE). Source: https://github.com/VladimirKovalevus/nexus-player
"""

STREAMING_README = """# streaming

Buffering and downloads for the Nexus player.

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

- [Nexus Player](https://github.com/VladimirKovalevus/nexus-player) - the application
"""

NOTICE_SECTIONS = {
    "Symphonia": """## vendor/Symphonia

- Upstream: https://github.com/pdeljanov/Symphonia, branch rel-0.5, commit bbaf23c
- Licence: MPL-2.0. The modified source files are in vendor/Symphonia.
- Changes: symphonia/src/lib.rs registers the libopus decoder;
  symphonia-format-ogg/src/demuxer.rs fixes a seek panic.
""",
    "symphonia-adapters": """## vendor/symphonia-adapters

- Upstream: https://github.com/aschey/symphonia-adapters, tag symphonia-adapter-libopus-v0.2.6 (ac436ed)
- Licence: MIT or Apache-2.0
- Changes: crates/symphonia-adapter-libopus/Cargo.toml spells out workspace fields.
""",
    "rsoundcloud": """## vendor/rsoundcloud

- Upstream: https://github.com/barthofu/rsoundcloud, commit d7196a5
- Licence: MIT, Copyright (c) 2025 Bartholomé Gili
- Changes: src/api/tracks.rs, src/client.rs, new src/utils/serde_helpers.rs.
""",
    "bandcamp": """## vendor/bandcamp

- Upstream: https://github.com/fabi321/bandcamp, tag v0.3.4 (bb4864c)
- Licence: MIT, Copyright (c) 2025 Fabian Wunsch
- Changes: src/search.rs makes the genre optional.
""",
    "butterchurn": """## vendor/butterchurn

- Upstream: https://github.com/jberg/butterchurn, commit 956cf4e
- Licence: MIT, Copyright (c) 2013-2018 Jordan Berg
- Changes: src/audio/audioProcessor.js, src/visualizer.js and src/rendering/renderer.js add a
  raw audio input mode; raw.md documents it.
""",
}

NOTICES_TAIL = """## Other components

- Visualizer presets come from the npm package butterchurn-presets (MIT).
- Fonts are IBM Plex, from @fontsource/ibm-plex-sans (OFL-1.1) and
  @fontsource/ibm-plex-mono (OFL-1.1).
- The logo, logo.png, is AI-generated.
- Other Rust dependencies are fetched from crates.io and npm dependencies from the npm
  registry, each under its own licence.
"""


def notices(sections=None, tail=NOTICES_TAIL):
    sections = NOTICE_SECTIONS if sections is None else sections
    return "# Third-party notices\n\n" + "\n".join(sections.values()) + "\n" + tail


LOCK_LICENSES = {
    "butterchurn-presets": "MIT",
    "@fontsource/ibm-plex-sans": "OFL-1.1",
    "@fontsource/ibm-plex-mono": "OFL-1.1",
}

RAW_MD = """# Butterchurn Raw Audio Interface

This interface lets Butterchurn run where there is no direct access to `AudioContext` (for example, when audio data comes from a Rust, C++ or Node.js backend).

## 1. Initialization

Pass `null` as the first argument when creating the visualizer. This stops Butterchurn from trying to connect to the Web Audio API.

```javascript
import butterchurn from 'butterchurn';

const canvas = document.getElementById('canvas');
const visualizer = butterchurn.createVisualizer(null, canvas, {
  width: 800,
  height: 600,
  pixelRatio: window.devicePixelRatio,
  textureRatio: 1,
});
```

## 2. Data format (audioData)

The `renderRaw` method expects an object with three `Uint8Array` arrays of **1024** bytes each.

*   **Values**: from `0` to `255`.
*   **Silence**: `128`.
*   **Length**: exactly `1024` (matches the internal `fftSize` / 2).

### If your backend (Rust) produces Float32 (-1.0 ... 1.0):
Convert the samples like this before sending them to JS:
`uint8_sample = (float_sample * 127.0 + 128.0) as u8`

## 3. Calling the renderer

Call `renderRaw` in your main loop (requestAnimationFrame or a callback from Rust).

```javascript
function onAudioFrame(dataFromRust, deltaTime) {
  const audioData = {
    timeByteArray:  dataFromRust.mixed,  // Uint8Array(1024)
    timeByteArrayL: dataFromRust.left,   // Uint8Array(1024)
    timeByteArrayR: dataFromRust.right   // Uint8Array(1024)
  };

  // deltaTime is the time in seconds since the previous frame (for example, 0.016)
  visualizer.renderRaw(audioData, deltaTime);
}
```

## 4. Why deltaTime (elapsedTime) matters

Butterchurn uses time to compute motion (rotation, zoom, pulsing).
- If `deltaTime` is not passed, it uses `performance.now()`.
- If you render video or your backend does not run in real time, **always** pass an exact `deltaTime` so the animation stays smooth and in sync with the music.

## 5. Example with mock data

```javascript
const mockData = new Uint8Array(1024).fill(128);

function loop() {
  // Fill with random data for testing
  for(let i=0; i<1024; i++) mockData[i] = Math.floor(Math.random() * 256);

  visualizer.renderRaw({
    timeByteArray:  mockData,
    timeByteArrayL: mockData,
    timeByteArrayR: mockData
  }, 1/60);

  requestAnimationFrame(loop);
}
```
"""

CI_YML = f"""name: CI

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  gitleaks:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@{CHECKOUT}
      - name: Install gitleaks
        run: |
          curl -sSfL -o gitleaks.tar.gz https://github.com/gitleaks/gitleaks/releases/download/v{GITLEAKS}/gitleaks_{GITLEAKS}_linux_x64.tar.gz
          echo "{DIGEST}  gitleaks.tar.gz" | sha256sum -c -
          tar -xzf gitleaks.tar.gz -C "$RUNNER_TEMP" gitleaks
      - name: Scan the checkout
        run: '"$RUNNER_TEMP/gitleaks" dir . --no-banner --redact --exit-code 1'

  build:
    runs-on: ubuntu-24.04
    env:
      RUSTUP_TOOLCHAIN: {RUST}
    steps:
      - uses: actions/checkout@{CHECKOUT}
      - name: Install system packages
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \\
            clang mold cmake pkg-config \\
            libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \\
            libjavascriptcoregtk-4.1-dev libasound2-dev
      - name: Install the Rust toolchain
        run: rustup toolchain install {RUST} --profile minimal --component rustfmt,clippy
      - name: Create the frontend placeholder
        run: |
          mkdir -p nexus-player-frontend/dist
          printf '<!doctype html>\\n' > nexus-player-frontend/dist/index.html
      - name: Format
        run: cargo fmt -p nexus-player-backend -p nexus-player-frontend -p streaming -- --check
      - name: Clippy
        run: cargo clippy -p nexus-player-backend -p nexus-player-frontend -p streaming --all-targets --locked --no-deps -- -D warnings
      - name: Tests
        run: cargo test -p nexus-player-backend -p nexus-player-frontend -p streaming --locked
      - name: File sizes
        run: python3 scripts/check_file_sizes.py
"""

AUTHORS = '["Vladimir Kovalev <vladkoval805@gmail.com>"]'
REPOSITORY = "https://github.com/VladimirKovalevus/nexus-player"


def with_package_keys(manifest, description=None):
    """A base manifest with the R8 keys written into its [package] table."""
    lines = manifest.split("\n")
    index = lines.index("[package]") + 1
    lines = [line for line in lines if not line.startswith(("authors =", "description ="))]
    extra = [f'license = "MIT"', f"authors = {AUTHORS}", f'repository = "{REPOSITORY}"']
    if description:
        extra.append(f'description = "{description}"')
    lines[index:index] = extra
    return "\n".join(lines)


def metadata_json(overrides=None):
    import json

    overrides = overrides or {}
    packages = []
    for name in ("nexus-player-backend", "nexus-player-frontend", "streaming", "rsoundcloud"):
        package = {
            "name": name,
            "license": "MIT",
            "authors": ["Vladimir Kovalev <vladkoval805@gmail.com>"],
            "repository": REPOSITORY,
        }
        package.update(overrides.get(name, {}))
        packages.append(package)
    return json.dumps({"packages": packages})
