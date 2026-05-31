use crossbeam::channel::{bounded, Receiver, Sender};
use reqwest::Client;
use std::{sync::Arc, thread, time::Duration, u64};

use crate::{
    buffers::chunked::ChunkedBuffer,
    downloader::{self, DownloadCommand, DEFAULT_CAPACITY, MAX_RETRIES},
    error::StreamingError,
    state::types::DEFAULT_USER_AGENT,
};

pub struct RangeDownloader {
    command_tx: Sender<DownloadCommand>,
    thread: Option<thread::JoinHandle<()>>,
}

impl RangeDownloader {
    pub fn new(url: String, buffer: Arc<ChunkedBuffer>) -> Self {
        let (tx, rx) = bounded(DEFAULT_CAPACITY);

        let thread = thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create tokio runtime: {}", e);
                    return;
                }
            };

            rt.block_on(RangeDownloader::download_loop(url, buffer, rx))
        });
        RangeDownloader {
            command_tx: tx,
            thread: Some(thread),
        }
    }
    pub fn start(&self) {
        let _ = self.command_tx.send(DownloadCommand::Start);
    }
    pub fn pause(&self) {
        let _ = self.command_tx.send(DownloadCommand::Pause);
    }
    pub fn seek_to(&self, offset: u64) {
        let _ = self.command_tx.send(DownloadCommand::Seek(offset));
    }

    pub fn stop(&self) {
        let _ = self.command_tx.send(DownloadCommand::Stop);
    }

    pub async fn download_chunk(
        client: &Client,
        url: &str,
        start: u64,
        end: u64,
    ) -> Result<Vec<u8>, StreamingError> {
        let range = format!("bytes={}-{}", start, end);
        let resp = client
            .get(url)
            .header("Range", range)
            .send()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(StreamingError::Http(format!("Status: {}", resp.status())));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| StreamingError::Io(e.to_string()))?;

        Ok(bytes.to_vec())
    }

    async fn download_loop(
        url: String,
        buffer: Arc<ChunkedBuffer>,
        command_rx: Receiver<DownloadCommand>,
    ) {
        let client = match Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(DEFAULT_USER_AGENT)
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Failed to build HTTP client: {}", e);
                return;
            }
        };

        let head = match client.head(&url).send().await {
            Ok(response) => {
                if let Some(size) = response.headers().get("content-length") {
                    if let Ok(length_str) = size.to_str() {
                        if let Ok(size) = length_str.parse::<u64>() {
                            buffer.set_total_size(size);
                        }
                    }
                }
            }
            Err(e) => {
                panic!("Client head err:{}", e);
            }
        };
        let mut current_offset = 0u64;
        let mut should_continue = true;
        let mut stopped = false;
        let mut retries = 0;

        while should_continue {
            match command_rx.recv_timeout(tokio::time::Duration::from_millis(10)) {
                Ok(DownloadCommand::Seek(offset)) => current_offset = offset,
                Ok(DownloadCommand::Stop) => should_continue = false,
                Ok(DownloadCommand::Start) => {
                    stopped = false;
                }
                Ok(DownloadCommand::Pause) => {
                    stopped = true;
                }
                Err(_) => {}
            }

            if !stopped {
                let chunk_index = current_offset / buffer.chunk_size() as u64;

                if buffer.has_chunk(chunk_index) {
                    current_offset += buffer.chunk_size();
                    continue;
                }
                let result = RangeDownloader::download_chunk(
                    &client,
                    &url,
                    current_offset,
                    current_offset + buffer.chunk_size() - 1,
                )
                .await;
                match result {
                    Ok(chunk) => {
                        buffer.write_chunk(chunk_index, chunk);
                        current_offset += buffer.chunk_size();
                        retries = 0;
                    }
                    Err(_) => {
                        if retries >= MAX_RETRIES {
                            break;
                        }
                        retries += 1;
                    }
                }
                if current_offset > buffer.total_size().unwrap_or(u64::MAX) {
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}

impl Drop for RangeDownloader {
    fn drop(&mut self) {
        self.stop();

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod mockito_tests {
    use super::*;
    use mockito::{Server, ServerGuard};

    #[tokio::test]
    async fn test_download_chunk_success() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/test-file")
            .match_header("Range", "bytes=0-65535")
            .with_status(206)
            .with_header("Content-Range", "bytes 0-65535/200000")
            .with_body(vec![0u8; 65536])
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/test-file", server.url());

        let result = RangeDownloader::download_chunk(&client, &url, 0, 65535).await;

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 65536);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_chunk_with_offset() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/test-file")
            .match_header("Range", "bytes=65536-131071")
            .with_status(206)
            .with_header("Content-Range", "bytes 65536-131071/200000")
            .with_body(vec![1u8; 65536])
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/test-file", server.url());

        let result = RangeDownloader::download_chunk(&client, &url, 65536, 131071).await;

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 65536);
        assert_eq!(data[0], 1);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_chunk_not_found() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/not-found")
            .with_status(404)
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/not-found", server.url());

        let result = RangeDownloader::download_chunk(&client, &url, 0, 65535).await;

        assert!(result.is_err());
        match result {
            Err(StreamingError::Http(msg)) => {
                assert!(msg.contains("404"));
            }
            _ => panic!("Expected HTTP error"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_chunk_server_error() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/error")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/error", server.url());

        let result = RangeDownloader::download_chunk(&client, &url, 0, 65535).await;

        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_chunk_empty_response() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/empty")
            .match_header("Range", "bytes=0-65535")
            .with_status(206)
            .with_body(vec![])
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/empty", server.url());

        let result = RangeDownloader::download_chunk(&client, &url, 0, 65535).await;

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 0);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_multiple_sequential_chunks() {
        let mut server = Server::new_async().await;

        let mock1 = server
            .mock("GET", "/file")
            .match_header("Range", "bytes=0-65535")
            .with_status(206)
            .with_body(vec![0u8; 65536])
            .create_async()
            .await;

        let mock2 = server
            .mock("GET", "/file")
            .match_header("Range", "bytes=65536-131071")
            .with_status(206)
            .with_body(vec![1u8; 65536])
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/file", server.url());

        let result1 = RangeDownloader::download_chunk(&client, &url, 0, 65535).await;
        assert!(result1.is_ok());
        assert_eq!(result1.unwrap().len(), 65536);

        let result2 = RangeDownloader::download_chunk(&client, &url, 65536, 131071).await;
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap().len(), 65536);

        mock1.assert_async().await;
        mock2.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_chunk_range_header() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/file")
            .match_header("Range", "bytes=1024-2047")
            .with_status(206)
            .with_body(vec![42u8; 1024])
            .create_async()
            .await;

        let client = Client::new();
        let url = format!("{}/file", server.url());

        let result = RangeDownloader::download_chunk(&client, &url, 1024, 2047).await;

        assert!(result.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_loop_stop() {
        let mut server = Server::new_async().await;

        let head_mock = server
            .mock("HEAD", "/test")
            .with_status(200)
            .with_header("Content-Length", "200000")
            .create_async()
            .await;

        let chunk_mock = server
            .mock("GET", "/test")
            .with_status(206)
            .with_body(vec![0u8; 65536])
            .expect_at_most(2)
            .create_async()
            .await;

        let buffer = Arc::new(ChunkedBuffer::new(false));
        let url = format!("{}/test", server.url());

        let downloader = RangeDownloader::new(url, buffer.clone());
        downloader.start();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        downloader.stop();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        assert!(!buffer.has_chunk(2));

        head_mock.assert_async().await;
    }
}
