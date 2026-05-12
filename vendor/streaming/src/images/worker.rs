use std::sync::Arc;

use tokio::sync::watch;

use super::{resize, Inner, Job};
use crate::error::StreamingError;

/// The single image worker.
///
/// One worker is the point: the whole app issues at most one image request at a
/// time, so caching a library's covers adds exactly one connection on top of the
/// audio manager's own single slot.
pub(crate) async fn run(inner: Arc<Inner>) {
    let mut pause_rx = inner.pause_tx.subscribe();

    loop {
        let job = next_job(&inner).await;

        // Yield to audio before every fetch, not just at the top of the loop:
        // a long queue must not outrun a download that started meanwhile.
        await_resume(&inner, &mut pause_rx).await;

        let outcome = process(&inner, &job).await;

        match outcome {
            Ok(path) => {
                tracing::trace!(url = %job.url, variant = job.variant.as_str(), "cached artwork");
                inner.release(&job, false);
                inner.notify_ready(&job, path);
            }
            Err(e) => {
                tracing::debug!(url = %job.url, variant = job.variant.as_str(), error = %e, "artwork fetch failed");
                inner.release(&job, true);
            }
        }
    }
}

async fn next_job(inner: &Arc<Inner>) -> Job {
    loop {
        if let Some(job) = inner.take_next() {
            return job;
        }
        // `Notify` holds a permit, so a request landing between the check above
        // and this await is not lost.
        inner.wake.notified().await;
    }
}

/// Block until the gate is clear, then sit out a grace period — restarting if the
/// gate re-arms during it. Mirrors the idle grace the audio event loop applies to
/// its own low-priority lane.
async fn await_resume(inner: &Arc<Inner>, pause_rx: &mut watch::Receiver<bool>) {
    loop {
        loop {
            let paused = *pause_rx.borrow_and_update();
            if !paused {
                break;
            }
            if pause_rx.changed().await.is_err() {
                return;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(inner.cfg.grace) => return,
            changed = pause_rx.changed() => {
                if changed.is_err() {
                    return;
                }
            }
        }
    }
}

async fn process(inner: &Arc<Inner>, job: &Job) -> Result<std::path::PathBuf, StreamingError> {
    // It may have been written while this job sat in the queue — by the other
    // variant's pass, or by a previous run of the app.
    if let Some(path) = inner.store.path_if_present(&job.key, job.variant) {
        return Ok(path);
    }

    let source = fetch(inner, &job.url).await?;

    let target = inner.cfg.size_for(job.variant);
    let quality = inner.cfg.quality;

    // Decode + resample is CPU work measured in tens of milliseconds; keep it off
    // the runtime's worker threads.
    let encoded = tokio::task::spawn_blocking(move || resize::to_square_webp(&source, target, quality))
        .await
        .map_err(|e| StreamingError::ImageDecode(format!("resize task failed: {e}")))??;

    inner.store.write(&job.key, job.variant, &encoded).await
}

async fn fetch(inner: &Arc<Inner>, url: &str) -> Result<Vec<u8>, StreamingError> {
    let response = inner
        .client
        .get(url)
        .send()
        .await
        .map_err(|e| StreamingError::Http(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(StreamingError::Http(format!("{status} for {url}")));
    }

    let limit = inner.cfg.max_source_bytes;
    if let Some(len) = response.content_length() {
        if len > limit {
            return Err(StreamingError::DownloadFailed(format!(
                "artwork too large: {len} bytes exceeds {limit}"
            )));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| StreamingError::Http(e.to_string()))?;

    // Servers lie about (or omit) content-length, so check what actually arrived.
    if bytes.len() as u64 > limit {
        return Err(StreamingError::DownloadFailed(format!(
            "artwork too large: {} bytes exceeds {limit}",
            bytes.len()
        )));
    }

    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use crate::images::{ImageCache, ImageCacheConfig, ImageVariant};
    use image::{DynamicImage, RgbImage};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn png(size: u32) -> Vec<u8> {
        let img = RgbImage::from_pixel(size, size, image::Rgb([10, 120, 220]));
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn cache(root: std::path::PathBuf) -> ImageCache {
        let mut cfg = ImageCacheConfig::new(root);
        // Keep the tests quick; the grace period is behaviour, not correctness.
        cfg.grace = Duration::from_millis(1);
        ImageCache::new(cfg)
    }

    /// Poll until `f` holds, so the tests don't depend on worker timing.
    async fn eventually(mut f: impl FnMut() -> bool) -> bool {
        for _ in 0..200 {
            if f() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        false
    }

    #[tokio::test]
    async fn fetches_resizes_and_caches() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/cover.png")
            .with_status(200)
            .with_header("content-type", "image/png")
            .with_body(png(1000))
            .expect(1)
            .create_async()
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path().to_path_buf());
        let url = format!("{}/cover.png", server.url());

        assert!(cache.path_if_cached(&url, ImageVariant::Thumb).is_none());
        cache.request(&url, ImageVariant::Thumb);

        assert!(
            eventually(|| cache.path_if_cached(&url, ImageVariant::Thumb).is_some()).await,
            "image never landed in the cache"
        );

        let path = cache.path_if_cached(&url, ImageVariant::Thumb).unwrap();
        let decoded = image::load_from_memory(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 256));

        // A second request is served from disk without touching the network.
        cache.request(&url, ImageVariant::Thumb);
        tokio::time::sleep(Duration::from_millis(100)).await;
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn a_paused_cache_issues_no_request() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/cover.png")
            .with_status(200)
            .with_body(png(300))
            .expect(0)
            .create_async()
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path().to_path_buf());
        cache.set_paused(true);

        let url = format!("{}/cover.png", server.url());
        cache.request(&url, ImageVariant::Thumb);

        tokio::time::sleep(Duration::from_millis(300)).await;
        mock.assert_async().await;
        assert!(cache.path_if_cached(&url, ImageVariant::Thumb).is_none());

        // Releasing the gate lets the queued job through.
        let mock = server
            .mock("GET", "/cover.png")
            .with_status(200)
            .with_body(png(300))
            .create_async()
            .await;
        cache.set_paused(false);

        assert!(eventually(|| cache.path_if_cached(&url, ImageVariant::Thumb).is_some()).await);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn on_ready_reports_the_local_path() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/cover.png")
            .with_status(200)
            .with_body(png(400))
            .create_async()
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path().to_path_buf());

        let hits = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&hits);
        cache.set_on_ready(Arc::new(move |_url, variant, path| {
            assert_eq!(variant, ImageVariant::Thumb);
            assert!(path.is_file());
            seen.fetch_add(1, Ordering::SeqCst);
        }));

        cache.request(&format!("{}/cover.png", server.url()), ImageVariant::Thumb);

        assert!(eventually(|| hits.load(Ordering::SeqCst) == 1).await);
    }

    #[tokio::test]
    async fn a_failing_url_is_not_retried_immediately() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/missing.png")
            .with_status(404)
            .expect(1)
            .create_async()
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path().to_path_buf());
        let url = format!("{}/missing.png", server.url());

        cache.request(&url, ImageVariant::Thumb);
        assert!(eventually(|| cache.pending_len() == 0).await);

        // Scrolling back over a broken cover must not re-hammer the server.
        for _ in 0..5 {
            cache.request(&url, ImageVariant::Thumb);
        }
        tokio::time::sleep(Duration::from_millis(150)).await;

        mock.assert_async().await;
        assert!(cache.path_if_cached(&url, ImageVariant::Thumb).is_none());
    }

    #[tokio::test]
    async fn oversized_sources_are_refused() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/huge.png")
            .with_status(200)
            .with_body(png(600))
            .create_async()
            .await;

        let dir = tempfile::tempdir().unwrap();
        let mut cfg = ImageCacheConfig::new(dir.path().to_path_buf());
        cfg.grace = Duration::from_millis(1);
        cfg.max_source_bytes = 64;
        let cache = ImageCache::new(cfg);

        let url = format!("{}/huge.png", server.url());
        cache.request(&url, ImageVariant::Thumb);

        assert!(eventually(|| cache.pending_len() == 0).await);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(cache.path_if_cached(&url, ImageVariant::Thumb).is_none());
    }

    #[tokio::test]
    async fn variants_are_cached_independently() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/cover.png")
            .with_status(200)
            .with_body(png(1000))
            .expect(2)
            .create_async()
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path().to_path_buf());
        let url = format!("{}/cover.png", server.url());

        cache.request(&url, ImageVariant::Thumb);
        cache.request(&url, ImageVariant::Large);

        assert!(eventually(|| cache.path_if_cached(&url, ImageVariant::Large).is_some()).await);
        assert!(eventually(|| cache.path_if_cached(&url, ImageVariant::Thumb).is_some()).await);

        let large = cache.path_if_cached(&url, ImageVariant::Large).unwrap();
        let decoded = image::load_from_memory(&std::fs::read(&large).unwrap()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (512, 512));
    }

    #[tokio::test]
    async fn clear_empties_the_store() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/cover.png")
            .with_status(200)
            .with_body(png(300))
            .create_async()
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path().to_path_buf());
        let url = format!("{}/cover.png", server.url());

        cache.request(&url, ImageVariant::Thumb);
        assert!(eventually(|| cache.path_if_cached(&url, ImageVariant::Thumb).is_some()).await);

        cache.clear().await.unwrap();
        assert!(cache.path_if_cached(&url, ImageVariant::Thumb).is_none());
    }
}
