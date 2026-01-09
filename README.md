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
