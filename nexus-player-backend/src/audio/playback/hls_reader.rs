use crate::audio::types::MediaSource;
use std::io::{Read, Seek, SeekFrom};
use streaming::multi_downloader::source::HlsSource;
use tracing::info;

#[derive(Debug, Clone)]
pub struct HlsReaderAdapter {
    source: HlsSource,
    position: u64,
}

impl HlsReaderAdapter {
    pub fn new(source: HlsSource) -> Self {
        Self {
            source,
            position: 0,
        }
    }
}

impl Read for HlsReaderAdapter {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.source.read(buf) {
            Ok(n) => {
                self.position += n as u64;
                Ok(n)
            }
            Err(e) => Err(e),
        }
    }
}

impl Seek for HlsReaderAdapter {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let total_size = self.source.estimate_total_size().unwrap_or(0);
        let new_pos = match pos {
            SeekFrom::Start(p) => p,
            SeekFrom::End(p) => {
                if total_size == 0 {
                    return Err(std::io::Error::other("Unknown stream size"));
                }
                if p >= 0 {
                    total_size.saturating_add(p as u64)
                } else {
                    total_size.saturating_sub(p.unsigned_abs())
                }
            }
            SeekFrom::Current(p) => {
                if p >= 0 {
                    self.position.saturating_add(p as u64)
                } else {
                    self.position.saturating_sub(p.unsigned_abs())
                }
            }
        };

        if new_pos != self.position {
            info!(
                "HlsReaderAdapter seek: {} -> {} (total: {})",
                self.position, new_pos, total_size
            );
            if let Err(e) = self.source.seek_to_byte_approx(new_pos) {
                return Err(std::io::Error::other(e.to_string()));
            }
            self.position = new_pos;
        }

        Ok(self.position)
    }
}

impl symphonia::core::io::MediaSource for HlsReaderAdapter {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.source.estimate_total_size()
    }
}

impl MediaSource for HlsReaderAdapter {
    fn clone_box(&self) -> std::io::Result<Box<dyn MediaSource>> {
        Ok(Box::new(self.clone()))
    }

    fn get_waveforms(&self) -> Vec<crate::audio::types::Waveform> {
        self.source
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
