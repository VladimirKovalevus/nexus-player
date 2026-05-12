# 🎵 Nexus Audio Player

Modern cross-platform audio player built with Rust and Vue.js

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Vue.js](https://img.shields.io/badge/vue.js-%2335495e.svg?style=flat&logo=vuedotjs&logoColor=%234FC08D)](https://vuejs.org/)
[![Tauri](https://img.shields.io/badge/tauri-%2324C8DB.svg?style=flat&logo=tauri&logoColor=%23FFFFFF)](https://tauri.app/)

## 🌟 Features

- 🎧 **High-quality audio playback** - powered by CPAL (Cross-Platform Audio Library)
- ♪ **Advanced audio decoding** - using Symphonia decoder
- <img src="https://img.shields.io/badge/Bandcamp-629aa9?style=flat&logo=bandcamp&logoColor=white" height="14"/> **Bandcamp streaming** - direct audio streaming from Bandcamp
- <img src="https://img.shields.io/badge/SoundCloud-FF3300?style=flat&logo=soundcloud&logoColor=white" height="14"/> **SoundCloud streaming** - play tracks from SoundCloud
- 📺 **HLS streaming support** - seamless HTTP Live Streaming
- ⚙️ **Cross-platform** - Windows, macOS, Linux support
- ⚡ **Lightweight & Fast** - powered by Tauri framework

## 📦 Project Structure

This is a monorepo containing:

```
nexus-monorepo/
├── nexus-player-backend/   # Rust audio processing backend
│   ├── src/
│   └── Cargo.toml
│
├── nexus-player-frontend/   # Vue.js + Tauri frontend
│   ├── src/                 # Vue application source
│   ├── src-tauri/           # Tauri configuration
│   └── package.json
│
└── Cargo.toml              # Workspace configuration
```

## Getting Started

### Prerequisites

- **Rust** (1.70 or later) - [Install Rust](https://www.rust-lang.org/tools/install)
- **Node.js** (18 or later) - [Install Node.js](https://nodejs.org/)
- **pnpm** or **npm** - Package manager
- **System dependencies** for Tauri - [Platform-specific guide](https://tauri.app/v1/guides/getting-started/prerequisites)

### Linux: WebKitGTK environment flags

On Linux, Tauri renders the UI through WebKitGTK, which has known issues with some GPU drivers and Wayland compositors. If the app misbehaves, try the environment variables below. Only set a flag if you actually have the matching symptom — most of them disable hardware acceleration and increase CPU usage.

| Variable | When to use it |
|----------|----------------|
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | Blank/white or black window, or `Failed to create GBM buffer` in the logs. Most common on **NVIDIA proprietary drivers** and on Wayland. Try this one first. |
| `WEBKIT_DISABLE_COMPOSITING_MODE=1` | Flickering, rendering artifacts, or frozen/laggy animations when the DMA-BUF flag alone didn't help. Turns off accelerated compositing entirely, so animations and the visualizer will use more CPU. |
| `__NV_DISABLE_EXPLICIT_SYNC=1` | **NVIDIA + Wayland** (driver 555+): the app crashes with `Error 71 (Protocol error) dispatching to Wayland display`. |
| `GDK_BACKEND=x11` | Wayland-specific problems that persist: broken window dragging/resizing, missing decorations, protocol errors. Forces the app to run through XWayland. |
| `LIBGL_ALWAYS_SOFTWARE=1` | Virtual machines or systems without a working GPU driver. Full software rendering — last resort, slow. |

Usage:

```bash
# one-off, in development
WEBKIT_DISABLE_DMABUF_RENDERER=1 npm run tauri dev

# several flags can be combined
WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11 nexus-player-frontend
```

To make a flag permanent, export it in your shell profile (`~/.bashrc`, `~/.zshrc`) or add it to the `Exec=` line of the app's `.desktop` file, e.g. `Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 nexus-player-frontend`.

`nexus-player-frontend/install.sh` does this automatically: it detects NVIDIA / Wayland / VM and writes the matching flags into the generated `.desktop` entry. Override the choice with `WEBKIT_FLAGS="GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1" ./install.sh`, or disable it with `WEBKIT_FLAGS="" ./install.sh`.

## Contributing

Contributions are welcome! Please check out our [Contributing Guidelines](CONTRIBUTING.md) for details.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Powered by [Vue.js](https://vuejs.org/)
- Audio backend: [CPAL](https://github.com/RustAudio/cpal)
- Audio decoding: [Symphonia](https://github.com/pdeljanov/Symphonia)


## Feature Status

### Streaming Capabilities
- [x] Range-based streaming
- [x] HLS streaming support

### Platform Integrations
- [x] Bandcamp integration
- [x] SoundCloud integration
- [ ] YouTube (Work in Progress)
- [ ] VK Music (Work in Progress)

---

<div align="center">
Made with ❤️ 
</div>
