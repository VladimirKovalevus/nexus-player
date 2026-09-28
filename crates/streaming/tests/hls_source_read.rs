use std::io::{ErrorKind, Read};
use std::sync::Arc;

use streaming::buffers::hls::HlsBuffer;
use streaming::multi_downloader::source::hls::HlsSource;
use streaming::state::types::ManagerCommand;
use tokio::sync::mpsc;

type Rx = mpsc::UnboundedReceiver<ManagerCommand>;

fn source_over(buffer: Arc<HlsBuffer>) -> (HlsSource, Rx) {
    let (tx, rx) = mpsc::unbounded_channel();
    (HlsSource::new(buffer, 7, Arc::new(tx)), rx)
}

fn buffer_with(
    segments: &[Option<Vec<u8>>],
    init_segment: Option<Vec<u8>>,
) -> Arc<HlsBuffer> {
    let buffer = Arc::new(HlsBuffer::new(false));
    buffer.init(segments.len(), vec![1.0; segments.len()], init_segment);
    buffer.set_container("opus".to_string());
    for (i, seg) in segments.iter().enumerate() {
        if let Some(data) = seg {
            buffer.write_chunk(i as u64, data.clone());
        }
    }
    buffer
}

fn read_all(source: &mut HlsSource) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = source.read(&mut chunk).expect("read failed");
        if n == 0 {
            return out;
        }
        out.extend_from_slice(&chunk[..n]);
    }
}

fn three_segments() -> Vec<Option<Vec<u8>>> {
    vec![
        Some(vec![1, 2, 3, 4, 5]),
        Some(vec![6, 7, 8]),
        Some(vec![9, 10, 11, 12, 13, 14, 15]),
    ]
}

#[test]
fn read_across_segment_boundary_returns_bytes_in_order() {
    let (mut source, _rx) = source_over(buffer_with(&three_segments(), None));
    // A buffer larger than the first segment must not skip or repeat bytes.
    let mut collected = Vec::new();
    let mut buf = [0u8; 4];
    loop {
        let n = source.read(&mut buf).expect("read failed");
        if n == 0 {
            break;
        }
        collected.extend_from_slice(&buf[..n]);
    }
    assert_eq!(collected, (1u8..=15).collect::<Vec<_>>());
}

#[test]
fn read_with_init_segment_keeps_init_before_every_segment() {
    let init = vec![0xAA, 0xBB];
    let segments = vec![Some(vec![1, 2, 3]), Some(vec![4, 5])];
    let (mut source, _rx) = source_over(buffer_with(&segments, Some(init)));
    assert_eq!(
        read_all(&mut source),
        vec![0xAA, 0xBB, 1, 2, 3, 0xAA, 0xBB, 4, 5]
    );
}

#[test]
fn read_of_segment_not_loaded_yet_would_block() {
    let segments = vec![Some(vec![1, 2]), None, Some(vec![5, 6])];
    let (mut source, _rx) = source_over(buffer_with(&segments, None));
    let mut buf = [0u8; 2];
    assert_eq!(source.read(&mut buf).expect("first segment"), 2);
    let err = source.read(&mut buf).expect_err("segment 1 is missing");
    assert_eq!(err.kind(), ErrorKind::WouldBlock);
}

#[test]
fn read_before_any_segment_arrives_would_block() {
    let segments = vec![None, None];
    let (mut source, _rx) = source_over(buffer_with(&segments, None));
    let mut buf = [0u8; 8];
    let err = source.read(&mut buf).expect_err("nothing loaded");
    assert_eq!(err.kind(), ErrorKind::WouldBlock);
}

#[test]
fn read_past_last_segment_returns_zero() {
    let (mut source, _rx) = source_over(buffer_with(&three_segments(), None));
    let _ = read_all(&mut source);
    let mut buf = [0u8; 16];
    assert_eq!(source.read(&mut buf).expect("eof"), 0);
    assert_eq!(source.read(&mut buf).expect("eof again"), 0);
}

#[test]
fn read_resumes_after_missing_segment_arrives() {
    let buffer = buffer_with(&[Some(vec![1, 2]), None], None);
    let (mut source, _rx) = source_over(buffer.clone());
    let mut buf = [0u8; 8];
    assert_eq!(source.read(&mut buf).expect("first"), 2);
    assert_eq!(
        source.read(&mut buf).expect_err("not yet").kind(),
        ErrorKind::WouldBlock
    );
    buffer.write_chunk(1, vec![3, 4, 5]);
    assert_eq!(source.read(&mut buf).expect("second"), 3);
    assert_eq!(&buf[..3], &[3, 4, 5]);
    assert_eq!(source.read(&mut buf).expect("eof"), 0);
}

#[test]
fn empty_destination_buffer_reads_zero_bytes() {
    let (mut source, _rx) = source_over(buffer_with(&three_segments(), None));
    assert_eq!(source.read(&mut []).expect("empty read"), 0);
}

#[test]
fn many_small_and_odd_reads_match_one_large_read() {
    let init = Some(vec![9, 9, 9]);
    let segments: Vec<Option<Vec<u8>>> = (0..5u8)
        .map(|i| Some((0..(10 + i as usize * 3)).map(|b| b as u8 ^ i).collect()))
        .collect();

    let (mut whole, _rx1) = source_over(buffer_with(&segments, init.clone()));
    let expected = read_all(&mut whole);
    assert!(!expected.is_empty());

    for size in [1usize, 3, 7, 13] {
        let (mut source, _rx) = source_over(buffer_with(&segments, init.clone()));
        let mut got = Vec::new();
        let mut buf = vec![0u8; size];
        loop {
            let n = source.read(&mut buf).expect("read failed");
            if n == 0 {
                break;
            }
            assert!(n <= size);
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got, expected, "read size {size}");
    }

    // std::io::Read path through a boxed reader of the same source
    let (source, _rx) = source_over(buffer_with(&segments, init));
    struct R(HlsSource);
    impl Read for R {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(b)
        }
    }
    let mut via_read = Vec::new();
    R(source).read_to_end(&mut via_read).expect("read_to_end");
    assert_eq!(via_read, expected);
}
