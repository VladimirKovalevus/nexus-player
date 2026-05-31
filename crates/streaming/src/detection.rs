use crate::{
    error::StreamingError,
    hls::{parser::parse_m3u8, segment::HlsPlaylist},
    state::types::DEFAULT_USER_AGENT,
};

fn with_bandcamp_headers(request: reqwest::RequestBuilder, url: &str) -> reqwest::RequestBuilder {
    if url.contains("bandcamp.com") || url.contains("bcbits.com") {
        return request
            .header("Referer", "https://bandcamp.com/")
            .header("Origin", "https://bandcamp.com")
            .header("Sec-Fetch-Site", "same-site")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "audio");
    }
    request
}

#[derive(Debug)]
pub enum StreamType {
    Range {
        url: String,
        size: Option<u64>,
    },
    Hls {
        playlist_url: String,
        playlist: Option<HlsPlaylist>,
    },
    Sequential {
        url: String,
    },
}

fn extract_bandcamp_stream_url(html: &str) -> Option<String> {
    // Bandcamp often keeps direct stream links in escaped JSON fields on track pages.
    let candidates = ["\"mp3-128\":\"", "\"mp3-v0\":\"", "\"mp3-320\":\""];

    for marker in candidates {
        if let Some(start_idx) = html.find(marker) {
            let value_start = start_idx + marker.len();
            let rest = &html[value_start..];
            if let Some(end_rel) = rest.find('"') {
                let mut url = rest[..end_rel].to_string();
                url = url.replace("\\/", "/");
                url = url.replace("\\u0026", "&");

                if url.starts_with("http://") || url.starts_with("https://") {
                    return Some(url);
                }
            }
        }
    }

    None
}

pub async fn detect_stream_type(
    url: &str,
    user_agent: Option<String>,
) -> Result<StreamType, StreamingError> {
    let ua = user_agent.unwrap_or_else(|| DEFAULT_USER_AGENT.to_string());

    // Explicitly use rustls to avoid native-tls (Windows SChannel) being selected
    // when both TLS backends are compiled in via feature unification (e.g. another dependency pulls native-tls).
    // SChannel has a known JA3 fingerprint that triggers Cloudflare bot challenges.
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .user_agent(ua)
        .build()
        .map_err(|e| StreamingError::Http(e.to_string()))?;

    let mut current_url = url.to_string();
    let mut attempts = 0usize;

    tracing::debug!("[detect_stream_type] starting detection for url={}", url);

    loop {
        attempts += 1;

        tracing::debug!(
            "[detect_stream_type] attempt={} url={}",
            attempts,
            current_url
        );

        let request = client
            .get(&current_url)
            .header("Range", "bytes=0-0")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
            .header("Accept-Language", "en-US,en;q=0.9");
        let response = with_bandcamp_headers(request, &current_url)
            .send()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;
        let status = response.status();
        let headers = response.headers().clone();

        let content_type_str = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("<none>")
            .to_string();

        tracing::debug!(
            "[detect_stream_type] attempt={} status={} content-type={} accept-ranges={} content-range={} content-length={}",
            attempts,
            status,
            content_type_str,
            headers.get("accept-ranges").and_then(|v| v.to_str().ok()).unwrap_or("<none>"),
            headers.get("content-range").and_then(|v| v.to_str().ok()).unwrap_or("<none>"),
            headers.get("content-length").and_then(|v| v.to_str().ok()).unwrap_or("<none>"),
        );

        {
            let content_type_lower = content_type_str.to_lowercase();

            if content_type_lower.contains("mpegurl") {
                //Soundcloud instantly gives hls playlist
                //Maybe parse it instantly in the future
                let content = response
                    .text()
                    .await
                    .map_err(|e| StreamingError::Http(e.to_string()))?;

                tracing::debug!(
                    "[detect_stream_type] detected HLS playlist at url={}",
                    current_url
                );

                match parse_m3u8(&content, &current_url) {
                    Ok(playlist) => {
                        return Ok(StreamType::Hls {
                            playlist_url: current_url,
                            playlist: Some(playlist),
                        });
                    }
                    Err(_) => {
                        return Ok(StreamType::Hls {
                            playlist_url: current_url,
                            playlist: None,
                        });
                    }
                }
            }

            if content_type_lower.contains("text/html")
                && attempts == 1
                && current_url.contains("bandcamp.com")
            {
                let html = response
                    .text()
                    .await
                    .map_err(|e| StreamingError::Http(e.to_string()))?;

                tracing::debug!(
                    "[detect_stream_type] got HTML from bandcamp.com, trying to extract stream url (html_len={})",
                    html.len()
                );

                if let Some(stream_url) = extract_bandcamp_stream_url(&html) {
                    tracing::debug!(
                        "[detect_stream_type] extracted stream url from HTML: {}",
                        stream_url
                    );
                    current_url = stream_url;
                    continue;
                } else {
                    tracing::warn!("[detect_stream_type] could not extract stream url from bandcamp HTML (html snippet: {:?})", &html[..html.len().min(500)]);
                }
            }

            if content_type_lower.contains("text/html") {
                tracing::warn!(
                    "[detect_stream_type] got HTML response for url={} (attempt={}, content-type={})",
                    current_url, attempts, content_type_str
                );
            }
        }

        let supports_range =
            status == reqwest::StatusCode::PARTIAL_CONTENT || headers.contains_key("accept-ranges");

        let size = if let Some(content_range) = headers.get("content-range") {
            if let Ok(range_str) = content_range.to_str() {
                range_str
                    .split('/')
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
            } else {
                None
            }
        } else {
            headers
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
        };

        if supports_range {
            tracing::debug!(
                "[detect_stream_type] result=Range url={} size={:?}",
                current_url,
                size
            );
            return Ok(StreamType::Range {
                url: current_url,
                size,
            });
        }

        tracing::debug!("[detect_stream_type] result=Sequential url={}", current_url);
        return Ok(StreamType::Sequential { url: current_url });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_hls_stream() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/playlist.m3u8")
            .match_header("range", "bytes=0-0")
            .with_status(200)
            .with_header("content-type", "application/vnd.apple.mpegurl")
            .with_body("X")
            .create_async()
            .await;

        let url = format!("{}/playlist.m3u8", server.url());
        let result = detect_stream_type(&url, None).await.unwrap();

        mock.assert_async().await;

        match result {
            StreamType::Hls { playlist_url, .. } => {
                assert_eq!(playlist_url, url);
            }
            _ => panic!("Expected HLS stream type"),
        }
    }

    #[tokio::test]
    async fn test_detect_range_stream_with_partial_content() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/audio.mp3")
            .match_header("range", "bytes=0-0")
            .with_status(206)
            .with_header("content-type", "audio/mpeg")
            .with_header("content-range", "bytes 0-0/12345")
            .with_header("accept-ranges", "bytes")
            .with_body("X")
            .create_async()
            .await;

        let url = format!("{}/audio.mp3", server.url());
        let result = detect_stream_type(&url, None).await.unwrap();

        mock.assert_async().await;

        match result {
            StreamType::Range {
                url: result_url,
                size,
            } => {
                assert_eq!(result_url, url);
                assert_eq!(size, Some(12345));
            }
            _ => panic!("Expected Range stream type"),
        }
    }

    #[tokio::test]
    async fn test_detect_range_stream_with_accept_ranges_header() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/video.mp4")
            .match_header("range", "bytes=0-0")
            .with_status(200)
            .with_header("content-type", "video/mp4")
            .with_header("accept-ranges", "bytes")
            .with_header("content-length", "9999")
            .with_body(vec![0u8; 9999])
            .create_async()
            .await;

        let url = format!("{}/video.mp4", server.url());
        let result = detect_stream_type(&url, None).await.unwrap();

        mock.assert_async().await;

        match result {
            StreamType::Range {
                url: result_url,
                size,
            } => {
                assert_eq!(result_url, url);
                assert_eq!(size, Some(9999));
            }
            _ => panic!("Expected Range stream type"),
        }
    }

    #[tokio::test]
    async fn test_detect_sequential_stream() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/stream.dat")
            .match_header("range", "bytes=0-0")
            .with_status(200)
            .with_header("content-type", "application/octet-stream")
            .with_body("X")
            .create_async()
            .await;

        let url = format!("{}/stream.dat", server.url());
        let result = detect_stream_type(&url, None).await.unwrap();

        mock.assert_async().await;

        match result {
            StreamType::Sequential { url: result_url } => {
                assert_eq!(result_url, url);
            }
            _ => panic!("Expected Sequential stream type"),
        }
    }

    #[tokio::test]
    async fn test_detect_hls_with_different_content_type() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/stream.m3u8")
            .match_header("range", "bytes=0-0")
            .with_status(200)
            .with_header("content-type", "application/x-mpegURL")
            .with_body("X")
            .create_async()
            .await;

        let url = format!("{}/stream.m3u8", server.url());
        let result = detect_stream_type(&url, None).await.unwrap();

        mock.assert_async().await;

        match result {
            StreamType::Hls { playlist_url, .. } => {
                assert_eq!(playlist_url, url);
            }
            _ => panic!("Expected HLS stream type"),
        }
    }

    #[tokio::test]
    async fn test_detect_range_without_size() {
        let mut server = mockito::Server::new_async().await;

        let mock = server
            .mock("GET", "/file.bin")
            .match_header("range", "bytes=0-0")
            .with_status(206)
            .with_header("content-type", "application/octet-stream")
            .with_body("X")
            .create_async()
            .await;

        let url = format!("{}/file.bin", server.url());
        let result = detect_stream_type(&url, None).await.unwrap();

        mock.assert_async().await;

        match result {
            StreamType::Range {
                url: result_url,
                size,
            } => {
                assert_eq!(result_url, url);
                assert_eq!(size, Some(1));
            }
            _ => panic!("Expected Range stream type {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_detect_bandcamp_html_fallback_to_stream_url() {
        let mut server = mockito::Server::new_async().await;

        let stream_url = format!("{}/stream/test.mp3", server.url());
        let escaped_stream_url = stream_url.replace('/', "\\/");

        let page_mock = server
            .mock("GET", "/bandcamp.com/track/test")
            .match_header("range", "bytes=0-0")
            .with_status(200)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body(format!(
                r#"<html><script>var data={{"mp3-128":"{}"}}</script></html>"#,
                escaped_stream_url
            ))
            .create_async()
            .await;

        let stream_mock = server
            .mock("GET", "/stream/test.mp3")
            .match_header("range", "bytes=0-0")
            .with_status(206)
            .with_header("content-type", "audio/mpeg")
            .with_header("content-range", "bytes 0-0/4321")
            .with_header("accept-ranges", "bytes")
            .with_body("X")
            .create_async()
            .await;

        let url = format!("{}/bandcamp.com/track/test", server.url());
        let result = detect_stream_type(&url, None).await.unwrap();

        page_mock.assert_async().await;
        stream_mock.assert_async().await;

        match result {
            StreamType::Range {
                url: result_url,
                size,
            } => {
                assert_eq!(result_url, stream_url);
                assert_eq!(size, Some(4321));
            }
            _ => panic!("Expected Range stream type"),
        }
    }
}
