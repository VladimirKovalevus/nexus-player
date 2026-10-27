# Third-Party Notices

This project vendors five third-party source trees under `vendor/`. Each one is listed below with its upstream, the base it was taken from, its licence, and every local change. Licence texts live next to the code in each directory.

## vendor/Symphonia

- **Upstream:** https://github.com/pdeljanov/Symphonia
- **Base:** branch `rel-0.5`, commit `bbaf23c` (v0.5.5 plus 7 commits, 2026-01-07). It includes the unreleased `symphonia-codec-opus` and `symphonia-codec-wavpack` 0.0.1 crates from that branch.
- **Licence:** MPL-2.0 (Mozilla Public License 2.0). The full text is in the `LICENSE` file of each crate. The copyright holders are the Symphonia contributors listed in `vendor/Symphonia/CONTRIBUTORS`.
- **Use:** the Symphonia crates replace their crates.io versions through the root `Cargo.toml` `[patch.crates-io]` table, and the backend and streaming crates depend on `symphonia` as a path dependency.
- **Local changes.** The modified source files are in `vendor/Symphonia`:
  - `symphonia/Cargo.toml` and `symphonia/src/lib.rs` gain an `opus` feature that registers the libopus adapter's `OpusDecoder`.
  - `symphonia-format-ogg/src/demuxer.rs` has a seek fix: `phys_byte_range_end.unwrap()` is replaced with `unwrap_or(start_byte_pos - 1)`, plus a `} else {` reformatting.
  - The root `Cargo.toml` `[patch.crates-io]` table points these crates at `vendor/Symphonia`: `symphonia-core`, `symphonia-metadata`, `symphonia-bundle-flac`, `symphonia-bundle-mp3`, `symphonia-codec-aac`, `symphonia-codec-adpcm`, `symphonia-codec-alac`, `symphonia-codec-pcm`, `symphonia-codec-vorbis`, `symphonia-format-isomp4`, `symphonia-format-mkv`, `symphonia-format-ogg`, `symphonia-format-riff`.
  - Each crate carries its own copy of the MPL-2.0 `LICENSE`.

## vendor/symphonia-adapters

- **Upstream:** https://github.com/aschey/symphonia-adapters
- **Base:** tag `symphonia-adapter-libopus-v0.2.6` (`ac436ed`).
- **Licence:** dual licensed MIT or Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`). The MIT copyright line reads "Copyright (c) 2025 Austin Schey".
- **Use:** only the libopus crate is used, through Symphonia. The fdk-aac crate is unused and is not in `Cargo.lock`.
- **Local changes:** only `crates/symphonia-adapter-libopus/Cargo.toml`, where workspace-inherited fields are written out so the crate builds outside its workspace. There is no source change.

## vendor/rsoundcloud

- **Upstream:** https://github.com/barthofu/rsoundcloud
- **Base:** commit `d7196a5` (v0.2.4 plus 2 commits, 2026-02-04). Upstream has since released v0.2.5 and v0.2.6.
- **Licence:** MIT, Copyright (c) 2025 Bartholomé Gili.
- **Use:** a workspace dependency in the root `Cargo.toml`.
- **Local changes** (about 400 changed lines):
  - `src/api/tracks.rs`: stream URL resolution, including a function that returns the final HLS URL from a transcoding URL.
  - `src/client.rs`: `with_client_id`, `set_client_id`, and a different `client_id` extraction from script assets.
  - Smaller changes in `src/api/playlists.rs`, `src/models/playlist.rs`, `src/models/track.rs`, `src/models/user.rs`, `src/http/reqwest.rs` and `src/lib.rs`.
  - New `src/utils/serde_helpers.rs`.
  - New `examples/get_playlist_tracks.rs` and `examples/search_and_stream.rs`.
  - New `.envrc` (nix-direnv, no secret).

## vendor/bandcamp

- **Upstream:** https://github.com/fabi321/bandcamp
- **Base:** tag `v0.3.4` (`bb4864c`), equal to crates.io `bandcamp` 0.3.4 apart from the change below.
- **Licence:** MIT, Copyright (c) 2025 Fabian Wunsch.
- **Use:** the backend's path dependency. The streaming crate's dev-dependency uses the crates.io copy.
- **Local changes:** only `src/search.rs` (9 lines): `genre_name` became `Option<String>`, and the serde error is mapped without `snafu::ResultExt`. `python_bindings/` and `codegen.py` are upstream files.

## vendor/butterchurn

- **Upstream:** https://github.com/jberg/butterchurn
- **Base:** branch `master`, commit `956cf4e` (2026-01-02).
- **Licence:** MIT, Copyright (c) 2013-2018 Jordan Berg.
- **Use:** the frontend depends on it as a `file:` npm dependency.
- **Local changes:**
  - `src/audio/audioProcessor.js`, `src/visualizer.js` and `src/rendering/renderer.js` (about 190 lines) add a raw audio input mode, so the visualizer runs on samples passed in from the Rust backend with no `AudioContext` (null-safe analysers, `updateAudio` accepting frequency arrays).
  - New `raw.md`, written by the project's author, describes that interface.
  - New `package-lock.json`.
  - `dist/` is not tracked: it is gitignored and built from these sources by `npm run setup` (the vendor build).
  - `test/visual` (the jest + puppeteer harness) is present and unused by the package build; its image snapshots, `experiments/` and `preview.png` were removed.
  - `pnpm-lock.yaml` was removed.

## libopus (statically linked)

- **Upstream:** https://opus-codec.org/ (Xiph.Org Foundation)
- **Licence:** BSD-3-Clause.
- **Use:** libopus is compiled from source and statically linked through the `opusic-sys` crate, version 0.5.8, which `symphonia-adapter-libopus` pulls in. It is not vendored in this repository.
- **Local changes:** none.

```
Copyright 2001-2023 Xiph.Org, Skype Limited, Octasic,
                    Jean-Marc Valin, Timothy B. Terriberry,
                    CSIRO, Gregory Maxwell, Mark Borgerding,
                    Erik de Castro Lopo, Mozilla, Amazon

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:

- Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

- Neither the name of Internet Society, IETF or IETF Trust, nor the
names of specific contributors, may be used to endorse or promote
products derived from this software without specific prior written
permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## Other assets and dependencies

The visualizer presets come from the npm package `butterchurn-presets` 2.4.7, licensed MIT according to its metadata in `nexus-player-frontend/package-lock.json`.

The fonts are IBM Plex, from the npm packages `@fontsource/ibm-plex-sans` and `@fontsource/ibm-plex-mono` 5.3.0, licensed OFL-1.1 according to `nexus-player-frontend/package-lock.json`.

The logo, `logo.png`, is AI-generated.

All other Rust and npm dependencies are fetched from crates.io and npm under their own licences, as recorded in `Cargo.lock` and `nexus-player-frontend/package-lock.json`.
