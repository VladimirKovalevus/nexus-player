# 📥 Streaming

**Efficient download manager for audio streaming** - Range-based and HLS streaming download library for Nexus Audio Player

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/streaming?style=flat)](https://crates.io/crates/streaming)

## 🎯 Overview

A Rust library for efficient audio streaming and downloading with support for both traditional range-based HTTP streaming and HLS (HTTP Live Streaming) protocols. Built specifically for the Nexus Audio Player ecosystem.

## ✨ Features

- 📦 **Range-based streaming** - Efficient HTTP range request downloads
- 📺 **HLS streaming support** - Full HTTP Live Streaming protocol implementation
- 🔄 **Multi-threaded downloads** - Concurrent chunk downloading for better performance
- 🎵 **Audio format detection** - Automatic format detection with lofty
- 🎼 **Symphonia integration** - Optional symphonia support for decoding
- 🛡️ **Error handling** - Robust error handling with thiserror
- ⚡ **Async-first** - Built on Tokio async runtime

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
streaming = "0.1.0"

# With symphonia support
streaming = { version = "0.1.0", features = ["symphonia-support"] }
```

## 🚀 Quick Start

### Basic Range-based Download

```rust
use streaming::downloader::Downloader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://example.com/audio.mp3";
    let downloader = Downloader::new(url)?;

    // Download file
    downloader.download().await?;

    Ok(())
}
```

### HLS Streaming

```rust
use streaming::hls::HlsDownloader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let m3u8_url = "https://example.com/playlist.m3u8";
    let hls = HlsDownloader::new(m3u8_url)?;

    // Download and process HLS stream
    hls.download_all_segments().await?;

    Ok(())
}
```

### Multi-threaded Download Manager

```rust
use streaming::multi_downloader::MultiDownloader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://example.com/large-audio.flac";
    let manager = MultiDownloader::new(url, 4)?; // 4 concurrent connections

    // Download with progress tracking
    manager.download_with_progress(|progress| {
        println!("Downloaded: {}%", progress);
    }).await?;

    Ok(())
}
```

## 🏗️ Architecture

```
streaming/
├── buffers/           # Buffering and memory management
├── downloader/        # Core range-based downloader
├── hls/               # HLS protocol implementation
├── multi_downloader/  # Multi-threaded download manager
├── detection.rs       # Format and protocol detection
├── source.rs          # Abstract data source
├── state.rs           # Download state management
└── error.rs           # Error types
```

## 📚 Examples

The library includes several examples in the `examples/` directory:

- `download.rs` - Basic download example
- `parse_hls.rs` - HLS playlist parsing
- `manager_demo.rs` - Download manager demonstration
- `play_url.rs` - Stream and play audio (requires `symphonia-support`)
- `test_hls.rs` - HLS streaming test

Run an example:

```bash
cargo run --example download
cargo run --example parse_hls --features symphonia-support
```

## 🔧 Features Flags

- `symphonia-support` - Enable Symphonia decoder integration for audio playback

```toml
[dependencies]
streaming = { version = "0.1.0", features = ["symphonia-support"] }
```

## 🛠️ Tech Stack

- **Tokio** - Async runtime
- **Reqwest** - HTTP client with streaming support
- **Crossbeam** - Lock-free concurrency primitives
- **Lofty** - Audio metadata and format detection
- **Symphonia** - Audio decoding (optional)

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run with symphonia support
cargo test --features symphonia-support

# Run examples
cargo run --example manager_demo
```

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🔗 Related Projects

- [Nexus Audio Player](../../README.md) - The main audio player application
- [CPAL](https://github.com/RustAudio/cpal) - Cross-platform audio I/O
- [Symphonia](https://github.com/pdeljanov/Symphonia) - Audio decoding library

---

<div align="center">
Built for Nexus Audio Player with ❤️
</div>
