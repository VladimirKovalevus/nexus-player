use std::io::{Read, Seek, SeekFrom};
use streaming::multi_downloader::source::StreamingSourceV2;

pub struct StreamingMediaSource(pub StreamingSourceV2);

impl Read for StreamingMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for StreamingMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl std::fmt::Debug for StreamingMediaSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl crate::audio::types::MediaSource for StreamingMediaSource {
    fn clone_box(&self) -> Box<dyn crate::audio::types::MediaSource> {
        Box::new(StreamingMediaSource(self.0.clone()))
    }
}

impl symphonia::core::io::MediaSource for StreamingMediaSource {
    fn is_seekable(&self) -> bool {
        use streaming::multi_downloader::source::MediaSource;
        self.0.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        use streaming::multi_downloader::source::MediaSource;
        self.0.byte_len()
    }
}
