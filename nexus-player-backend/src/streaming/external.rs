use std::io::{Read, Seek, SeekFrom};
use streaming::multi_downloader::source::{HlsSource, StreamingSourceV2};

pub struct StreamingMediaSource(pub StreamingSourceV2);

pub enum StreamingSourceWrapper {
    Hls(HlsSource),
    Streaming(StreamingMediaSource),
    Local(Box<dyn crate::audio::types::MediaSource>),
}

impl StreamingSourceWrapper {
    pub fn from_source(source: streaming::multi_downloader::source::Source) -> Self {
        match source {
            streaming::multi_downloader::source::Source::Hls(hls) => {
                StreamingSourceWrapper::Hls(hls)
            }
            streaming::multi_downloader::source::Source::Streaming(s) => {
                StreamingSourceWrapper::Streaming(StreamingMediaSource(s))
            }
        }
    }

    pub async fn prepare(&self) -> Result<(), streaming::error::StreamingError> {
        match self {
            StreamingSourceWrapper::Hls(hls) => hls.prepare().await,
            StreamingSourceWrapper::Streaming(s) => s.0.prepare().await,
            StreamingSourceWrapper::Local(_) => Ok(()),
        }
    }

    pub fn play(
        self,
        engine: &crate::audio::engine::AudioEngine,
        track: crate::audio::types::Track,
    ) {
        match self {
            StreamingSourceWrapper::Streaming(source) => {
                engine.play_source(track, Box::new(source));
            }
            StreamingSourceWrapper::Hls(hls) => {
                engine.play_hls(track, hls);
            }
            StreamingSourceWrapper::Local(source) => {
                engine.play_source(track, source);
            }
        }
    }

    pub fn preload(
        self,
        engine: &crate::audio::engine::AudioEngine,
        track: crate::audio::types::Track,
    ) {
        match self {
            StreamingSourceWrapper::Streaming(source) => {
                engine.preload_source(track, Box::new(source));
            }
            StreamingSourceWrapper::Hls(hls) => {
                engine.preload_hls(track, hls);
            }
            StreamingSourceWrapper::Local(source) => {
                engine.preload_source(track, source);
            }
        }
    }
}

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

    fn get_waveforms(&self) -> Vec<crate::audio::types::Waveform> {
        self.0
            .get_waveforms()
            .into_iter()
            .map(|w| crate::audio::types::Waveform {
                data: w.data,
                start_time: w.start_time,
                end_time: w.end_time,
                byte_start: w.byte_start,
                byte_end: w.byte_end,
            })
            .collect()
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
