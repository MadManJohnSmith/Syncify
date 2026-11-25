//! DLX-05 (#46): medición offline de la portada repetida por pista.
//!
//! La línea base reproduce el `reqwest::get` por pista que había en las rutas
//! Qobuz, Tidal y MP3. La candidata pasa por la caché acotada single-flight.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinSet;

const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R',
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, b'I', b'D', b'A', b'T', 0x78, 0xDA, 0x63, 0x64, 0xF8, 0xCF, 0xC0,
    0x00, 0x00, 0x01, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, b'I', b'E',
    b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
];

struct CoverServer {
    base: String,
    requests: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl CoverServer {
    fn request_count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

impl Drop for CoverServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn servir_portada(listener: TcpListener, requests: Arc<AtomicUsize>, delay: Duration) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        requests.fetch_add(1, Ordering::SeqCst);
        let delay = if requests.load(Ordering::SeqCst) % 2 == 0 {
            delay
        } else {
            Duration::ZERO
        };
        tokio::spawn(async move {
            let mut request = [0_u8; 2048];
            if socket.read(&mut request).await.is_err() {
                return;
            }
            tokio::time::sleep(delay).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                PNG.len()
            );
            if socket.write_all(header.as_bytes()).await.is_err() {
                return;
            }
            let _ = socket.write_all(PNG).await;
        });
    }
}

async fn arrancar_servidor() -> CoverServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let requests = Arc::new(AtomicUsize::new(0));
    let task = tokio::spawn(servir_portada(
        listener,
        requests.clone(),
        Duration::from_millis(25),
    ));
    CoverServer {
        base,
        requests,
        task,
    }
}

async fn descargar_sin_cache(url: String) -> Vec<u8> {
    reqwest::get(url)
        .await
        .expect("respuesta local")
        .error_for_status()
        .expect("HTTP 200")
        .bytes()
        .await
        .expect("payload local")
        .to_vec()
}

async fn medir_baseline(pistas: usize) -> (usize, u128) {
    let server = arrancar_servidor().await;
    let url = format!("{}/album/cover.png", server.base);
    let inicio = Instant::now();
    let mut set = JoinSet::new();
    for _ in 0..pistas {
        let url = url.clone();
        set.spawn(async move { descargar_sin_cache(url).await });
    }
    let mut payloads = Vec::with_capacity(pistas);
    while let Some(result) = set.join_next().await {
        payloads.push(result.expect("tarea de portada"));
    }
    assert!(payloads.iter().all(|payload| payload == PNG));
    (server.request_count(), inicio.elapsed().as_millis())
}

#[tokio::test]
async fn baseline_sin_cache_repite_la_portada_por_pista() {
    let mut filas = Vec::new();
    for pistas in [2, 12] {
        let (peticiones, ms) = medir_baseline(pistas).await;
        assert_eq!(peticiones, pistas);
        filas.push(format!("{pistas} pistas: {peticiones} peticiones, {ms} ms"));
    }
    println!("DLX-05 baseline · {}", filas.join(" · "));
}
