//! Static cover-art retrieval shared by the download adapters.
//!
//! Callers keep ownership of their staging paths. This module only returns
//! immutable bytes; it never writes into a track's staging directory.

use reqwest::Client;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// A cover payload and whether the retrieval avoided a network request.
#[derive(Debug, Clone)]
pub struct CoverFetch {
    pub bytes: Arc<[u8]>,
    pub cache_hit: bool,
}

/// Best-effort cover retrieval failure. Audio delivery does not depend on it.
#[derive(Debug, Clone)]
pub enum CoverError {
    Request(String),
    Http(reqwest::StatusCode),
    Empty,
    Body(String),
    Cancelled,
}

impl std::fmt::Display for CoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(error) => write!(f, "cover request failed: {error}"),
            Self::Http(status) => write!(f, "cover HTTP status {status}"),
            Self::Empty => f.write_str("cover payload was empty"),
            Self::Body(error) => write!(f, "cover body failed: {error}"),
            Self::Cancelled => f.write_str("cover request cancelled"),
        }
    }
}

impl std::error::Error for CoverError {}

async fn wait_for_cancellation(token: Option<&CancellationToken>) {
    match token {
        Some(token) => token.cancelled().await,
        None => std::future::pending().await,
    }
}

/// Fetch one cover without caching. Kept as the baseline seam for DLX-05's
/// before/after measurement and used whenever resource identity is ambiguous.
pub async fn fetch_cover_uncached(
    client: &Client,
    url: &str,
    cancel_token: Option<&CancellationToken>,
) -> Result<Arc<[u8]>, CoverError> {
    let response = tokio::select! {
        biased;
        _ = wait_for_cancellation(cancel_token) => return Err(CoverError::Cancelled),
        response = client.get(url).send() => {
            response.map_err(|error| CoverError::Request(error.to_string()))?
        }
    };
    if !response.status().is_success() {
        return Err(CoverError::Http(response.status()));
    }
    let bytes = tokio::select! {
        biased;
        _ = wait_for_cancellation(cancel_token) => return Err(CoverError::Cancelled),
        bytes = response.bytes() => {
            bytes.map_err(|error| CoverError::Body(error.to_string()))?
        }
    };
    if bytes.is_empty() {
        return Err(CoverError::Empty);
    }
    Ok(Arc::from(bytes.as_ref()))
}

/// Production cover seam. DLX-05 characterization starts with one request per
/// track; the bounded single-flight cache is added behind this same interface.
pub async fn fetch_cover(
    client: &Client,
    url: &str,
    cancel_token: Option<&CancellationToken>,
) -> Result<CoverFetch, CoverError> {
    Ok(CoverFetch {
        bytes: fetch_cover_uncached(client, url, cancel_token).await?,
        cache_hit: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinSet;

    const COVER: &[u8] = b"synthetic-cover";

    struct CoverServer {
        base: String,
        requests: Arc<AtomicUsize>,
        task: tokio::task::JoinHandle<()>,
    }

    impl Drop for CoverServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn start_server(max_connections: usize) -> CoverServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = requests.clone();
        let task = tokio::spawn(async move {
            for _ in 0..max_connections {
                let Ok(Ok((mut socket, _))) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept()).await
                else {
                    break;
                };
                let counter = counter.clone();
                tokio::spawn(async move {
                    let mut request = [0u8; 2048];
                    if socket.read(&mut request).await.is_err() {
                        return;
                    }
                    counter.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        COVER.len()
                    );
                    if socket.write_all(header.as_bytes()).await.is_err() {
                        return;
                    }
                    let _ = socket.write_all(COVER).await;
                });
            }
        });
        CoverServer {
            base,
            requests,
            task,
        }
    }

    #[tokio::test]
    async fn characterization_two_and_twelve_tracks_fetch_once_per_track() {
        let client = Client::new();
        let staging = tempfile::tempdir().unwrap();
        for tracks in [2usize, 12] {
            let server = start_server(tracks).await;
            let url = format!("{}/album/cover.jpg", server.base);
            let started = std::time::Instant::now();
            let mut tasks = JoinSet::new();
            for _ in 0..tracks {
                let client = client.clone();
                let url = url.clone();
                tasks.spawn(async move { fetch_cover(&client, &url, None).await.unwrap() });
            }
            let mut delivered = 0usize;
            while let Some(result) = tasks.join_next().await {
                let cover = result.unwrap();
                assert_eq!(cover.bytes.as_ref(), COVER);
                assert!(!cover.cache_hit);
                let path = staging
                    .path()
                    .join(format!("{tracks}-{delivered}.cover.jpg"));
                tokio::fs::write(&path, cover.bytes.as_ref()).await.unwrap();
                assert_eq!(tokio::fs::read(&path).await.unwrap(), COVER);
                delivered += 1;
            }
            assert_eq!(delivered, tracks);
            assert_eq!(server.requests.load(Ordering::SeqCst), tracks);
            println!(
                "DLX-05 baseline tracks={tracks} requests={} network_bytes={} delivered_bytes={} elapsed_ms={}",
                server.requests.load(Ordering::SeqCst),
                tracks * COVER.len(),
                delivered * COVER.len(),
                started.elapsed().as_millis()
            );
        }
    }
}
