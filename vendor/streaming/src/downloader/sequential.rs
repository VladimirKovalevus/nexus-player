use crate::buffers::chunked::ChunkedBuffer;
use crossbeam::channel::{bounded, Sender};
use std::sync::Arc;

pub struct SequentialDownloader {
    url: String,
    buffer: Arc<ChunkedBuffer>,
    command_tx: Sender<()>,
}

impl SequentialDownloader {
    pub fn new(url: String, buffer: Arc<ChunkedBuffer>) -> Self {
        let (tx, _rx) = bounded(10);

        // TODO: Реализовать

        Self {
            url,
            buffer,
            command_tx: tx,
        }
    }

    pub fn start(&self) {
        // TODO
    }

    pub fn pause(&self) {
        // TODO
    }

    pub fn seek_to(&self, _offset: u64) {
        // TODO
    }

    pub fn stop(&self) {
        // TODO
    }
}
