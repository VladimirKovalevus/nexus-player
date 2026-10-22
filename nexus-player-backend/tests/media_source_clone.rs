use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use nexus_player_backend::audio::decoder::AudioDecoder;
use nexus_player_backend::audio::types::MediaSource;
use symphonia::core::io::MediaSource as SymphoniaSource;

/// A source whose clone always fails, like an exhausted fd table.
#[derive(Debug)]
struct FailingCloneSource {
    inner: Cursor<Vec<u8>>,
}

impl Read for FailingCloneSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for FailingCloneSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl SymphoniaSource for FailingCloneSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.inner.get_ref().len() as u64)
    }
}

impl MediaSource for FailingCloneSource {
    fn clone_box(&self) -> io::Result<Box<dyn MediaSource>> {
        Err(io::Error::other("too many open files"))
    }
}

#[test]
fn decoder_from_reader_returns_error_when_clone_fails() {
    let source = FailingCloneSource {
        inner: Cursor::new(vec![0u8; 4096]),
    };
    let result = AudioDecoder::from_reader(Box::new(source), None, 44100);
    assert!(result.is_err());
}

#[test]
fn file_clone_box_returns_ok_and_reads_same_bytes() {
    let mut tmp = tempfile::tempfile().expect("create temp file");
    tmp.write_all(b"hello clone").expect("write");
    tmp.seek(SeekFrom::Start(0)).expect("rewind");

    let clone = MediaSource::clone_box(&tmp);
    let mut clone = clone.expect("clone of a real file succeeds");
    let mut bytes = Vec::new();
    clone.seek(SeekFrom::Start(0)).expect("seek clone");
    clone.read_to_end(&mut bytes).expect("read clone");
    assert_eq!(bytes, b"hello clone");
}
