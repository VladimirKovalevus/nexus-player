use std::sync::Arc;

use crate::downloader::{
    hls::HlsDownloader, range::RangeDownloader, sequential::SequentialDownloader,
};

pub mod hls;
pub mod range;
pub mod sequential;

pub trait Downloader: Send + Sync {
    fn start(&self);
    fn pause(&self);
    fn seek_to(&self, offset: u64);
    fn stop(&self);
}

#[derive(Debug)]
pub enum DownloadCommand {
    Start,
    Pause,
    Seek(u64),
    Stop,
}

const DEFAULT_CAPACITY: usize = 10;
const MAX_RETRIES: usize = 3;

pub enum DownloaderHandle {
    Range(Arc<RangeDownloader>),
    Hls(Arc<HlsDownloader>),
    Sequential(Arc<SequentialDownloader>),
}

impl DownloaderHandle {
    pub fn start(&self) {
        match self {
            Self::Range(d) => d.start(),
            Self::Hls(d) => d.start(),
            Self::Sequential(d) => d.start(),
        }
    }

    pub fn pause(&self) {
        match self {
            Self::Range(d) => d.pause(),
            Self::Hls(d) => d.pause(),
            Self::Sequential(d) => d.pause(),
        }
    }

    pub fn seek_to(&self, offset: u64) {
        match self {
            Self::Range(d) => d.seek_to(offset),
            Self::Hls(d) => d.seek_to(offset),
            Self::Sequential(d) => d.seek_to(offset),
        }
    }

    pub fn stop(&self) {
        match self {
            Self::Range(d) => d.stop(),
            Self::Hls(d) => d.stop(),
            Self::Sequential(d) => d.stop(),
        }
    }
}

impl Drop for DownloaderHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
