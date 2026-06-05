// T2 R3: compile-only. This crate sits outside the library, so a path that stops being public
// fails to build. It holds no test, which keeps the set of test names unchanged.

type _DownloadManager = streaming::state::manager::DownloadManager;
type _BandwidthMonitor = streaming::state::manager::BandwidthMonitor;
type _PlaybackContext = streaming::state::manager::PlaybackContext;

type _HlsBuffer = streaming::buffers::hls::HlsBuffer;
type _HlsSequentialReader = streaming::buffers::hls::HlsSequentialReader;
type _HlsBufferReader = streaming::buffers::hls::HlsBufferReader;
