//! Static cover-art retrieval shared by the download adapters.
//!
//! Callers keep ownership of their staging paths. This module only returns
//! immutable bytes; it never writes into a track's staging directory.

use futures_util::StreamExt;
use reqwest::Client;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

const DEFAULT_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_CACHE_ENTRIES: usize = 64;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_IN_FLIGHT: usize = 64;
const MAX_COVER_BYTES: usize = 16 * 1024 * 1024;

/// A cover payload and whether retrieval reused an earlier result.
#[derive(Debug, Clone)]
pub struct CoverFetch {
    pub bytes: Arc<[u8]>,
    pub cache_hit: bool,
}

/// The access and edition axes that identify one static cover resource.
///
/// `None` from [`CoverIdentity::new`] means the resource is ambiguous and must
/// not enter the shared cache.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct CoverIdentity {
    provider: String,
    account_id: i64,
    edition_id: String,
}

impl CoverIdentity {
    pub fn new(provider: &str, account_id: i64, edition_id: &str) -> Option<Self> {
        let provider = provider.trim();
        let edition_id = edition_id.trim();
        if provider.is_empty() || account_id <= 0 || edition_id.is_empty() {
            return None;
        }
        Some(Self {
            provider: provider.to_ascii_lowercase(),
            account_id,
            edition_id: edition_id.to_string(),
        })
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct CoverKey {
    url: String,
    identity: CoverIdentity,
}

impl CoverKey {
    fn new(url: &str, identity: CoverIdentity) -> Result<Self, CoverError> {
        let mut url = reqwest::Url::parse(url.trim()).map_err(|_| CoverError::AmbiguousIdentity)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(CoverError::AmbiguousIdentity);
        }
        // Fragments are client-side and never reach the server. Query strings
        // stay intact because they may select a size or carry access scope.
        url.set_fragment(None);
        Ok(Self {
            url: url.to_string(),
            identity,
        })
    }
}

/// Best-effort cover retrieval failure. Audio delivery does not depend on it.
#[derive(Debug, Clone)]
pub enum CoverError {
    Request(String),
    Http(reqwest::StatusCode),
    Empty,
    Body(String),
    Cancelled,
    AmbiguousIdentity,
    TooLarge { limit: usize },
    FlightClosed,
}

impl std::fmt::Display for CoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(error) => write!(f, "cover request failed: {error}"),
            Self::Http(status) => write!(f, "cover HTTP status {status}"),
            Self::Empty => f.write_str("cover payload was empty"),
            Self::Body(error) => write!(f, "cover body failed: {error}"),
            Self::Cancelled => f.write_str("cover request cancelled"),
            Self::AmbiguousIdentity => f.write_str("cover identity is ambiguous"),
            Self::TooLarge { limit } => write!(f, "cover payload exceeds {limit} bytes"),
            Self::FlightClosed => f.write_str("cover retrieval ended without a result"),
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

async fn download_cover(
    client: &Client,
    url: &str,
    max_bytes: usize,
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
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(CoverError::TooLarge { limit: max_bytes });
    }

    let capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
        .min(max_bytes);
    let mut bytes = Vec::with_capacity(capacity);
    let mut stream = response.bytes_stream();
    loop {
        let chunk = tokio::select! {
            biased;
            _ = wait_for_cancellation(cancel_token) => return Err(CoverError::Cancelled),
            chunk = stream.next() => {
                let Some(chunk) = chunk else {
                    break;
                };
                chunk.map_err(|error| CoverError::Body(error.to_string()))?
            }
        };
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(CoverError::TooLarge { limit: max_bytes });
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err(CoverError::Empty);
    }
    Ok(Arc::from(bytes))
}

/// Fetch one cover without caching. This is also the safe path for ambiguous
/// resource identity and for callers whose cancellation must abort the request.
pub async fn fetch_cover_uncached(
    client: &Client,
    url: &str,
    cancel_token: Option<&CancellationToken>,
) -> Result<Arc<[u8]>, CoverError> {
    download_cover(client, url, MAX_COVER_BYTES, cancel_token).await
}

struct CacheConfig {
    ttl: Duration,
    max_entries: usize,
    max_bytes: usize,
    max_cover_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            ttl: DEFAULT_TTL,
            max_entries: MAX_CACHE_ENTRIES,
            max_bytes: MAX_CACHE_BYTES,
            max_cover_bytes: MAX_COVER_BYTES,
        }
    }
}

struct ReadyCover {
    value: CoverFetch,
    inserted_at: Instant,
}

struct CacheState {
    ready: HashMap<CoverKey, ReadyCover>,
    lru: VecDeque<CoverKey>,
    in_flight: HashMap<CoverKey, watch::Receiver<Option<Result<CoverFetch, CoverError>>>>,
    bytes: usize,
}

impl CacheState {
    fn new() -> Self {
        Self {
            ready: HashMap::new(),
            lru: VecDeque::new(),
            in_flight: HashMap::new(),
            bytes: 0,
        }
    }

    fn touch(&mut self, key: &CoverKey) {
        if let Some(position) = self.lru.iter().position(|candidate| candidate == key) {
            self.lru.remove(position);
        }
        self.lru.push_back(key.clone());
    }

    fn purge_expired(&mut self, now: Instant, ttl: Duration) {
        self.ready
            .retain(|_, entry| now.saturating_duration_since(entry.inserted_at) < ttl);
        self.lru.retain(|key| self.ready.contains_key(key));
        self.bytes = self
            .ready
            .values()
            .map(|entry| entry.value.bytes.len())
            .sum();
    }

    fn remove_ready(&mut self, key: &CoverKey) {
        if let Some(entry) = self.ready.remove(key) {
            self.bytes -= entry.value.bytes.len();
        }
        self.lru.retain(|candidate| candidate != key);
    }

    fn insert_ready(&mut self, key: CoverKey, value: CoverFetch, config: &CacheConfig) {
        let size = value.bytes.len();
        if config.max_entries == 0 || config.max_bytes == 0 || size > config.max_bytes {
            return;
        }
        self.remove_ready(&key);
        while !self.ready.is_empty()
            && (self.ready.len() >= config.max_entries
                || self.bytes.saturating_add(size) > config.max_bytes)
        {
            let Some(oldest) = self.lru.pop_front() else {
                self.ready.clear();
                self.bytes = 0;
                break;
            };
            self.remove_ready(&oldest);
        }
        self.bytes += size;
        self.lru.push_back(key.clone());
        self.ready.insert(
            key,
            ReadyCover {
                value,
                inserted_at: Instant::now(),
            },
        );
    }
}

struct CacheInner {
    config: CacheConfig,
    state: Mutex<CacheState>,
    request_slots: Arc<Semaphore>,
}

/// Bounded, process-local cache for immutable static cover bytes.
#[derive(Clone)]
pub struct CoverCache {
    inner: Arc<CacheInner>,
}

impl CoverCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CacheInner {
                config: CacheConfig::default(),
                state: Mutex::new(CacheState::new()),
                request_slots: Arc::new(Semaphore::new(MAX_IN_FLIGHT)),
            }),
        }
    }

    /// Fetch a cover, sharing one in-flight request and then reusing its bytes
    /// until TTL/eviction. A missing or invalid identity bypasses the cache but
    /// still shares the global request budget. Cancellation stops this caller's
    /// wait, never another caller's retrieval.
    pub async fn fetch(
        &self,
        client: &Client,
        url: &str,
        identity: Option<&CoverIdentity>,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<CoverFetch, CoverError> {
        if cancel_token.is_some_and(CancellationToken::is_cancelled) {
            return Err(CoverError::Cancelled);
        }
        let Some(identity) = identity else {
            return self.fetch_uncached_limited(client, url, cancel_token).await;
        };
        let Ok(key) = CoverKey::new(url, identity.clone()) else {
            return self.fetch_uncached_limited(client, url, cancel_token).await;
        };

        match self.start(key, client.clone(), cancel_token).await? {
            Start::Hit(value) => Ok(value),
            Start::Flight {
                receiver,
                cache_hit,
            } => {
                let result = wait_for_flight(receiver, cancel_token).await;
                result.map(|mut value| {
                    value.cache_hit = cache_hit;
                    value
                })
            }
        }
    }

    async fn fetch_uncached_limited(
        &self,
        client: &Client,
        url: &str,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<CoverFetch, CoverError> {
        let _permit = self.acquire_request_slot(cancel_token).await?;
        fetch_cover_uncached(client, url, cancel_token)
            .await
            .map(|bytes| CoverFetch {
                bytes,
                cache_hit: false,
            })
    }

    async fn acquire_request_slot(
        &self,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<OwnedSemaphorePermit, CoverError> {
        tokio::select! {
            biased;
            _ = wait_for_cancellation(cancel_token) => Err(CoverError::Cancelled),
            permit = self.inner.request_slots.clone().acquire_owned() => {
                permit.map_err(|_| CoverError::FlightClosed)
            }
        }
    }

    fn lookup_locked(state: &mut CacheState, key: &CoverKey, ttl: Duration) -> Option<Start> {
        state.purge_expired(Instant::now(), ttl);
        if let Some(entry) = state.ready.get(key) {
            let bytes = entry.value.bytes.clone();
            state.touch(key);
            return Some(Start::Hit(CoverFetch {
                bytes,
                cache_hit: true,
            }));
        }
        state.in_flight.get(key).map(|receiver| Start::Flight {
            receiver: receiver.clone(),
            cache_hit: true,
        })
    }

    fn lookup(&self, key: &CoverKey) -> Option<Start> {
        let mut state = self.inner.state.lock().expect("cover cache mutex poisoned");
        Self::lookup_locked(&mut state, key, self.inner.config.ttl)
    }

    async fn start(
        &self,
        key: CoverKey,
        client: Client,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<Start, CoverError> {
        if let Some(start) = self.lookup(&key) {
            return Ok(start);
        }
        let permit = self.acquire_request_slot(cancel_token).await?;
        let (receiver, sender) = {
            let mut state = self.inner.state.lock().expect("cover cache mutex poisoned");
            // Recheck and insert under the same lock. Separate lookup/insert
            // steps let two first callers overwrite each other's receiver.
            if let Some(start) = Self::lookup_locked(&mut state, &key, self.inner.config.ttl) {
                return Ok(start);
            }
            let (sender, receiver) = watch::channel(None);
            state.in_flight.insert(key.clone(), receiver.clone());
            (receiver, sender)
        };

        let cache = self.clone();
        tokio::spawn(async move {
            cache.complete_flight(key, client, sender, permit).await;
        });
        Ok(Start::Flight {
            receiver,
            cache_hit: false,
        })
    }

    async fn complete_flight(
        &self,
        key: CoverKey,
        client: Client,
        sender: watch::Sender<Option<Result<CoverFetch, CoverError>>>,
        _permit: OwnedSemaphorePermit,
    ) {
        let result = download_cover(&client, &key.url, self.inner.config.max_cover_bytes, None)
            .await
            .map(|bytes| CoverFetch {
                bytes,
                cache_hit: false,
            });

        {
            let mut state = self.inner.state.lock().expect("cover cache mutex poisoned");
            state.in_flight.remove(&key);
            if let Ok(value) = &result {
                state.insert_ready(key, value.clone(), &self.inner.config);
            }
        }
        let _ = sender.send(Some(result));
    }
}

impl Default for CoverCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl CoverCache {
    fn with_limits(
        ttl: Duration,
        max_entries: usize,
        max_bytes: usize,
        max_cover_bytes: usize,
        max_in_flight: usize,
    ) -> Self {
        Self {
            inner: Arc::new(CacheInner {
                config: CacheConfig {
                    ttl,
                    max_entries,
                    max_bytes,
                    max_cover_bytes,
                },
                state: Mutex::new(CacheState::new()),
                request_slots: Arc::new(Semaphore::new(max_in_flight)),
            }),
        }
    }
}

enum Start {
    Hit(CoverFetch),
    Flight {
        receiver: watch::Receiver<Option<Result<CoverFetch, CoverError>>>,
        cache_hit: bool,
    },
}

async fn wait_for_flight(
    mut receiver: watch::Receiver<Option<Result<CoverFetch, CoverError>>>,
    cancel_token: Option<&CancellationToken>,
) -> Result<CoverFetch, CoverError> {
    loop {
        if let Some(result) = receiver.borrow_and_update().clone() {
            return result;
        }
        tokio::select! {
            biased;
            _ = wait_for_cancellation(cancel_token) => return Err(CoverError::Cancelled),
            changed = receiver.changed() => {
                if changed.is_err() && receiver.borrow().is_none() {
                    return Err(CoverError::FlightClosed);
                }
            }
        }
    }
}

fn shared_cache() -> &'static CoverCache {
    static CACHE: OnceLock<CoverCache> = OnceLock::new();
    CACHE.get_or_init(CoverCache::new)
}

/// Production cover seam. Supply an identity only when account and edition are
/// both known; otherwise retrieval deliberately remains uncached.
pub async fn fetch_cover(
    client: &Client,
    url: &str,
    identity: Option<&CoverIdentity>,
    cancel_token: Option<&CancellationToken>,
) -> Result<CoverFetch, CoverError> {
    shared_cache()
        .fetch(client, url, identity, cancel_token)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::{Barrier, Notify};
    use tokio::task::JoinSet;

    const COVER: &[u8] = b"synthetic-cover";

    #[derive(Clone)]
    struct ResponsePlan {
        delay: Duration,
        responses: Arc<Mutex<VecDeque<(u16, &'static [u8])>>>,
    }

    impl ResponsePlan {
        fn new(delay: Duration, responses: Vec<(u16, &'static [u8])>) -> Self {
            Self {
                delay,
                responses: Arc::new(Mutex::new(responses.into())),
            }
        }
    }

    struct CoverServer {
        base: String,
        requests: Arc<AtomicUsize>,
        started: Arc<Notify>,
        task: tokio::task::JoinHandle<()>,
    }

    impl Drop for CoverServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn start_server_with(
        max_connections: usize,
        delay: Duration,
        responses: Vec<(u16, &'static [u8])>,
    ) -> CoverServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(Notify::new());
        let plan = ResponsePlan::new(delay, responses);
        let counter = requests.clone();
        let request_started = started.clone();
        let task = tokio::spawn(async move {
            for _ in 0..max_connections {
                let Ok(Ok((mut socket, _))) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept()).await
                else {
                    break;
                };
                let counter = counter.clone();
                let request_started = request_started.clone();
                let plan = plan.clone();
                tokio::spawn(async move {
                    let mut request = [0u8; 2048];
                    if socket.read(&mut request).await.is_err() {
                        return;
                    }
                    counter.fetch_add(1, Ordering::SeqCst);
                    request_started.notify_one();
                    let (status, body) = plan
                        .responses
                        .lock()
                        .expect("response plan mutex poisoned")
                        .pop_front()
                        .unwrap_or((200, COVER));
                    tokio::time::sleep(plan.delay).await;
                    let reason = if status == 200 {
                        "OK"
                    } else {
                        "Service Unavailable"
                    };
                    let header = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    if socket.write_all(header.as_bytes()).await.is_err() {
                        return;
                    }
                    let _ = socket.write_all(body).await;
                });
            }
        });
        CoverServer {
            base,
            requests,
            started,
            task,
        }
    }

    async fn start_server(max_connections: usize) -> CoverServer {
        start_server_with(max_connections, Duration::from_millis(30), Vec::new()).await
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
                tasks.spawn(async move { fetch_cover(&client, &url, None, None).await.unwrap() });
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

    #[tokio::test]
    async fn different_access_contexts_editions_and_urls_never_share() {
        let server = start_server(5).await;
        let cache = CoverCache::new();
        let client = Client::new();
        let account_seven = CoverIdentity::new("test", 7, "album-1").unwrap();
        let account_eight = CoverIdentity::new("test", 8, "album-1").unwrap();
        let other_edition = CoverIdentity::new("test", 7, "album-2").unwrap();
        let other_provider = CoverIdentity::new("other", 7, "album-1").unwrap();
        let first_url = format!("{}/first.png", server.base);
        let second_url = format!("{}/second.png", server.base);
        let cases = [
            (first_url.clone(), account_seven.clone()),
            (first_url.clone(), account_eight),
            (first_url.clone(), other_edition),
            (first_url, other_provider),
            (second_url, account_seven),
        ];
        let mut tasks = JoinSet::new();
        for (url, identity) in cases {
            let cache = cache.clone();
            let client = client.clone();
            tasks.spawn(async move { cache.fetch(&client, &url, Some(&identity), None).await });
        }
        while let Some(result) = tasks.join_next().await {
            let cover = result.unwrap().unwrap();
            assert_eq!(cover.bytes.as_ref(), COVER);
            assert!(!cover.cache_hit);
        }
        assert_eq!(server.requests.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn ambiguous_identity_bypasses_cache() {
        assert!(CoverIdentity::new("", 7, "album-1").is_none());
        assert!(CoverIdentity::new("test", 0, "album-1").is_none());
        assert!(CoverIdentity::new("test", 7, " ").is_none());

        let server = start_server(2).await;
        let url = format!("{}/album.png", server.base);
        let cache = CoverCache::new();
        let client = Client::new();
        let mut tasks = JoinSet::new();
        for _ in 0..2 {
            let cache = cache.clone();
            let client = client.clone();
            let url = url.clone();
            tasks.spawn(async move { cache.fetch(&client, &url, None, None).await });
        }
        while let Some(result) = tasks.join_next().await {
            let cover = result.unwrap().unwrap();
            assert_eq!(cover.bytes.as_ref(), COVER);
            assert!(!cover.cache_hit);
        }
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn ambiguous_url_bypasses_cache_but_still_fetches() {
        let server = start_server(2).await;
        let url = server.base.replacen("http://", "http://user:password@", 1);
        let cache = CoverCache::new();
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();
        for _ in 0..2 {
            let cover = cache
                .fetch(&client, &url, Some(&identity), None)
                .await
                .unwrap();
            assert_eq!(cover.bytes.as_ref(), COVER);
            assert!(!cover.cache_hit);
        }
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn canonical_url_ignores_fragment_and_reuses_immutable_bytes() {
        let server = start_server(1).await;
        let url = format!("{}/album.png", server.base);
        let cache = CoverCache::new();
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();
        let first = cache
            .fetch(&client, &format!("{url}#first"), Some(&identity), None)
            .await
            .unwrap();
        let second = cache
            .fetch(&client, &format!("{url}#second"), Some(&identity), None)
            .await
            .unwrap();
        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert!(Arc::ptr_eq(&first.bytes, &second.bytes));
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn http_failure_is_not_cached() {
        let server = start_server_with(2, Duration::ZERO, vec![(503, b""), (200, COVER)]).await;
        let url = format!("{}/album.png", server.base);
        let cache = CoverCache::new();
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();
        let first = cache.fetch(&client, &url, Some(&identity), None).await;
        assert!(matches!(first, Err(CoverError::Http(status)) if status.as_u16() == 503));
        let second = cache
            .fetch(&client, &url, Some(&identity), None)
            .await
            .unwrap();
        assert_eq!(second.bytes.as_ref(), COVER);
        assert!(!second.cache_hit);
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn empty_payload_is_not_cached() {
        let server = start_server_with(2, Duration::ZERO, vec![(200, b""), (200, COVER)]).await;
        let url = format!("{}/album.png", server.base);
        let cache = CoverCache::new();
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();
        assert!(matches!(
            cache.fetch(&client, &url, Some(&identity), None).await,
            Err(CoverError::Empty)
        ));
        let second = cache
            .fetch(&client, &url, Some(&identity), None)
            .await
            .unwrap();
        assert_eq!(second.bytes.as_ref(), COVER);
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cancelling_one_waiter_does_not_cancel_shared_flight() {
        let server = start_server(1).await;
        let url = format!("{}/album.png", server.base);
        let cache = CoverCache::new();
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();
        let token = CancellationToken::new();
        let request_started = server.started.notified();
        let leader_cache = cache.clone();
        let leader_client = client.clone();
        let leader_url = url.clone();
        let leader_identity = identity.clone();
        let leader_token = token.clone();
        let leader = tokio::spawn(async move {
            leader_cache
                .fetch(
                    &leader_client,
                    &leader_url,
                    Some(&leader_identity),
                    Some(&leader_token),
                )
                .await
        });
        request_started.await;

        let waiter_cache = cache.clone();
        let waiter = tokio::spawn(async move {
            waiter_cache
                .fetch(&client, &url, Some(&identity), None)
                .await
        });
        token.cancel();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), leader)
                .await
                .unwrap()
                .unwrap(),
            Err(CoverError::Cancelled)
        ));
        let cover = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(cover.bytes.as_ref(), COVER);
        assert!(cover.cache_hit);
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn expiration_and_lru_eviction_refetch() {
        let server = start_server_with(4, Duration::ZERO, Vec::new()).await;
        let first_url = format!("{}/first.png", server.base);
        let second_url = format!("{}/second.png", server.base);
        let cache =
            CoverCache::with_limits(Duration::from_millis(20), 1, 1024, 1024, MAX_IN_FLIGHT);
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();

        cache
            .fetch(&client, &first_url, Some(&identity), None)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        let after_ttl = cache
            .fetch(&client, &first_url, Some(&identity), None)
            .await
            .unwrap();
        assert!(!after_ttl.cache_hit);
        cache
            .fetch(&client, &second_url, Some(&identity), None)
            .await
            .unwrap();
        let after_eviction = cache
            .fetch(&client, &first_url, Some(&identity), None)
            .await
            .unwrap();
        assert!(!after_eviction.cache_hit);
        assert_eq!(server.requests.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn cache_byte_budget_evicts_older_covers() {
        let server = start_server_with(3, Duration::ZERO, Vec::new()).await;
        let first_url = format!("{}/first.png", server.base);
        let second_url = format!("{}/second.png", server.base);
        let cache = CoverCache::with_limits(
            Duration::from_secs(60),
            4,
            COVER.len(),
            COVER.len(),
            MAX_IN_FLIGHT,
        );
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();

        cache
            .fetch(&client, &first_url, Some(&identity), None)
            .await
            .unwrap();
        cache
            .fetch(&client, &second_url, Some(&identity), None)
            .await
            .unwrap();
        let refetched = cache
            .fetch(&client, &first_url, Some(&identity), None)
            .await
            .unwrap();
        assert!(!refetched.cache_hit);
        assert_eq!(server.requests.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn simultaneous_first_callers_share_one_flight_with_spare_capacity() {
        let rounds = 25;
        let callers = 16;
        let server = start_server_with(rounds, Duration::from_millis(10), Vec::new()).await;
        let url = format!("{}/album.png", server.base);
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();

        for _ in 0..rounds {
            let cache = CoverCache::with_limits(Duration::from_secs(60), 4, 1024, 1024, callers);
            let barrier = Arc::new(Barrier::new(callers));
            let mut tasks = JoinSet::new();
            for _ in 0..callers {
                let cache = cache.clone();
                let client = client.clone();
                let url = url.clone();
                let identity = identity.clone();
                let barrier = barrier.clone();
                tasks.spawn(async move {
                    barrier.wait().await;
                    cache.fetch(&client, &url, Some(&identity), None).await
                });
            }
            let mut hits = 0;
            while let Some(result) = tasks.join_next().await {
                let cover = result.unwrap().unwrap();
                assert_eq!(cover.bytes.as_ref(), COVER);
                hits += usize::from(cover.cache_hit);
            }
            assert_eq!(hits, callers - 1);
        }
        assert_eq!(server.requests.load(Ordering::SeqCst), rounds);
    }

    #[tokio::test]
    async fn capacity_wait_preserves_single_flight_for_the_same_key() {
        let server = start_server_with(3, Duration::from_millis(500), Vec::new()).await;
        let first_url = format!("{}/first.png", server.base);
        let second_url = format!("{}/second.png", server.base);
        let cache = CoverCache::with_limits(Duration::from_secs(60), 4, 1024, 1024, 1);
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();
        let request_started = server.started.notified();
        let first_cache = cache.clone();
        let first_client = client.clone();
        let first_identity = identity.clone();
        let first = tokio::spawn(async move {
            first_cache
                .fetch(&first_client, &first_url, Some(&first_identity), None)
                .await
        });
        request_started.await;

        let mut waiting = JoinSet::new();
        for _ in 0..2 {
            let cache = cache.clone();
            let client = client.clone();
            let url = second_url.clone();
            let identity = identity.clone();
            waiting.spawn(async move { cache.fetch(&client, &url, Some(&identity), None).await });
        }
        first.await.unwrap().unwrap();
        let mut hits = 0;
        while let Some(result) = waiting.join_next().await {
            let cover = result.unwrap().unwrap();
            assert_eq!(cover.bytes.as_ref(), COVER);
            hits += usize::from(cover.cache_hit);
        }
        assert_eq!(hits, 1);
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn oversized_payload_is_rejected_and_not_cached() {
        let server = start_server(2).await;
        let url = format!("{}/album.png", server.base);
        let cache = CoverCache::with_limits(
            Duration::from_secs(60),
            4,
            1024,
            COVER.len().saturating_sub(1),
            MAX_IN_FLIGHT,
        );
        let client = Client::new();
        let identity = CoverIdentity::new("test", 7, "album-1").unwrap();
        for _ in 0..2 {
            assert!(matches!(
                cache.fetch(&client, &url, Some(&identity), None).await,
                Err(CoverError::TooLarge { .. })
            ));
        }
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    }
}
