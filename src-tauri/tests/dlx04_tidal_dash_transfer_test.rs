//! DLX-04 (#45): medición offline de la etapa de transferencia DASH de Tidal,
//! aislada del pipeline (sin BD, sin credenciales, sin red externa).
//!
//! Sirve por HTTP local el fixture fMP4-FLAC real (`tiny-fmp4-flac.m4a`,
//! init + 4 fragmentos moof+mdat) como manifiesto DASH y compara:
//! - la variante en serie (el bucle segmento a segmento que el brazo Tidal
//!   ejecuta hoy, invariante que #45 fija) contra
//! - la variante concurrente (techo que paralelizaría recuperar si el
//!   contrato del proveedor lo permitiera algún día).
//!
//! La comparación es la que pide la cola DLX («comparar p50/p95 de
//! transferencia en serie frente a la variante»); con retardo por segmento
//! `D`, la serie paga `init + N·D` y la variante concurrente `init + D`.
//! Nada de esto activa red externa ni toca `download/orchestrator.rs`.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use syncify_tauri_lib::download::tidal::TidalDownloader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinSet;

const FIXTURE: &str = "../crates/syncify-fmp4-flac/tests/fixtures/tiny-fmp4-flac.m4a";

/// Retardo por petición de segmento del servidor local. Con N segmentos la
/// serie paga N·D y la variante concurrente una sola D: la brecha medida es
/// el techo que #45 deja documentado sin paralelizar.
const DELAY_POR_SEGMENTO: Duration = Duration::from_millis(30);
const CORRIDAS: usize = 7;

/// Divide el fixture en (init, segmentos moof+mdat), igual que el test del
/// crate `syncify-fmp4-flac`.
fn split_segments(data: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut init: Vec<u8> = Vec::new();
    let mut segments: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<Vec<u8>> = None;
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        if size < 8 || pos + size > data.len() {
            break;
        }
        let typ = &data[pos + 4..pos + 8];
        let end = pos + size;
        match typ {
            b"moof" => current = Some(data[pos..end].to_vec()),
            b"mdat" => {
                if let Some(mut seg) = current.take() {
                    seg.extend_from_slice(&data[pos..end]);
                    segments.push(seg);
                }
            }
            _ => {
                if segments.is_empty() && current.is_none() {
                    init.extend_from_slice(&data[pos..end]);
                }
            }
        }
        pos = end;
    }
    (init, segments)
}

struct DashServer {
    base: String,
    handle: tokio::task::JoinHandle<()>,
    /// Rutas servidas en el orden exacto en que llegaron: es el registro que
    /// acredita «se piden y se consumen en orden, sin saltos».
    orden: Arc<Mutex<Vec<String>>>,
}

async fn servir_dash(
    listener: TcpListener,
    init: Arc<Vec<u8>>,
    segmentos: Arc<Vec<Vec<u8>>>,
    delay: Duration,
    max_conns: usize,
    orden: Arc<Mutex<Vec<String>>>,
) {
    for _ in 0..max_conns {
        let Ok(Ok((mut socket, _))) =
            tokio::time::timeout(Duration::from_secs(10), listener.accept()).await
        else {
            break;
        };
        let init = init.clone();
        let segmentos = segmentos.clone();
        let orden = orden.clone();
        tokio::spawn(async move {
            let mut request = [0u8; 2048];
            let Ok(n) = socket.read(&mut request).await else {
                return;
            };
            let head = String::from_utf8_lossy(&request[..n]).to_string();
            let ruta = head
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_string();
            orden.lock().unwrap().push(ruta.clone());

            let payload = if ruta.ends_with("/init") {
                Some((*init).clone())
            } else {
                ruta.rsplit("/seg_")
                    .next()
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|idx| segmentos.get(idx))
                    .map(|seg| seg.clone())
            };
            let Some(payload) = payload else {
                let _ = socket
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
                return;
            };
            if !ruta.ends_with("/init") && !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            if socket.write_all(header.as_bytes()).await.is_err() {
                return;
            }
            let _ = socket.write_all(&payload).await;
        });
    }
}

async fn arrancar_servidor() -> DashServer {
    let data = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE))
        .expect("fixture fMP4 presente");
    let (init, segmentos) = split_segments(&data);
    assert!(
        init.windows(4).any(|w| w == b"ftyp"),
        "el init del fixture es ISOBMFF"
    );
    assert!(!segmentos.is_empty(), "el fixture debe traer fragmentos");

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let orden = Arc::new(Mutex::new(Vec::new()));
    let handle = tokio::spawn(servir_dash(
        listener,
        Arc::new(init),
        Arc::new(segmentos),
        DELAY_POR_SEGMENTO,
        256,
        orden.clone(),
    ));
    DashServer { base, handle, orden }
}

fn spec_dash(base: &str, total: usize) -> String {
    format!("DASH_MANIFEST|{base}/init|{base}/seg_$Number$|{total}")
}

fn percentil(mut valores: Vec<u128>, p: f64) -> u128 {
    valores.sort_unstable();
    let idx = ((valores.len() as f64 - 1.0) * p).round() as usize;
    valores[idx.min(valores.len() - 1)]
}

/// Corrida en serie: el bucle real del brazo Tidal, contra el servidor local.
async fn correr_serie(total_segmentos: usize) -> Vec<u128> {
    let mut duraciones = Vec::with_capacity(CORRIDAS);
    for _ in 0..CORRIDAS {
        let server = arrancar_servidor().await;
        let dir = tempfile::tempdir().expect("tmp aislado");
        let salida = dir.path().join("track.flac");
        let downloader = TidalDownloader::new();
        let inicio = Instant::now();
        let bytes = downloader
            .download_audio_payload_with_progress(
                &spec_dash(&server.base, total_segmentos),
                &salida,
                |_, _, _| {},
            )
            .await
            .expect("descarga DASH en serie");
        duraciones.push(inicio.elapsed().as_millis());
        // El payload entregado es el FLAC nativo remuxado: la calidad no cambia.
        let final_bytes = tokio::fs::read(&salida).await.unwrap();
        assert!(final_bytes.starts_with(b"fLaC"), "FLAC nativo tras remux");
        assert!(bytes > 0);
        server.handle.abort();
    }
    duraciones
}

/// Variante concurrente (solo medición, nunca producción): pide los segmentos
/// a la vez y escribe en orden. Cuantifica el techo que paralelizar recuperar.
async fn correr_variante_concurrente(total_segmentos: usize) -> Vec<u128> {
    let client = reqwest::Client::new();
    let mut duraciones = Vec::with_capacity(CORRIDAS);
    for _ in 0..CORRIDAS {
        let server = arrancar_servidor().await;
        let dir = tempfile::tempdir().expect("tmp aislado");
        let salida = dir.path().join("track.flac");
        let inicio = Instant::now();
        let init = client
            .get(format!("{}/init", server.base))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();

        let mut set: JoinSet<(usize, Vec<u8>)> = JoinSet::new();
        for n in 1..=total_segmentos {
            let client = client.clone();
            let url = format!("{}/seg_{n}", server.base);
            set.spawn(async move {
                let bytes = client.get(url).send().await.unwrap().bytes().await.unwrap();
                (n, bytes.to_vec())
            });
        }
        let mut partes: Vec<(usize, Vec<u8>)> = Vec::with_capacity(total_segmentos);
        while let Some(res) = set.join_next().await {
            partes.push(res.expect("tarea de segmento"));
        }
        partes.sort_by_key(|(n, _)| *n);

        let mut fichero = tokio::fs::File::create(&salida).await.unwrap();
        fichero.write_all(&init).await.unwrap();
        for (_, parte) in &partes {
            fichero.write_all(parte).await.unwrap();
        }
        fichero.flush().await.unwrap();
        duraciones.push(inicio.elapsed().as_millis());

        let en_disco = tokio::fs::metadata(&salida).await.unwrap().len();
        let esperado = init.len() as u64 + partes.iter().map(|(_, p)| p.len() as u64).sum::<u64>();
        assert_eq!(en_disco, esperado, "payload completo en disco");
        server.handle.abort();
    }
    duraciones
}

#[tokio::test]
async fn transferencia_dash_serie_vs_variante_orden_y_medicion() {
    let (init_len, seg_len) = {
        let data = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)).unwrap();
        let (init, segs) = split_segments(&data);
        (init.len(), segs.len())
    };
    assert_eq!(seg_len, 4, "el fixture trae 4 fragmentos (init + 4)");

    // 1. Serie: orden exacto init -> seg_1..seg_N, sin saltos ni duplicados.
    //    Este es el invariante que #45 fija para el brazo Tidal.
    let server = arrancar_servidor().await;
    let dir = tempfile::tempdir().unwrap();
    let salida = dir.path().join("track.flac");
    TidalDownloader::new()
        .download_audio_payload_with_progress(
            &spec_dash(&server.base, seg_len),
            &salida,
            |_, _, _| {},
        )
        .await
        .expect("descarga en serie");
    let orden = server.orden.lock().unwrap().clone();
    let esperado: Vec<String> = std::iter::once("/init".to_string())
        .chain((1..=seg_len).map(|n| format!("/seg_{n}")))
        .collect();
    assert_eq!(
        orden, esperado,
        "los segmentos DASH se piden en orden, sin saltos"
    );
    server.handle.abort();

    // 2. Medición p50/p95: serie frente a variante concurrente.
    let serie = correr_serie(seg_len).await;
    let variante = correr_variante_concurrente(seg_len).await;

    let serie_p50 = percentil(serie.clone(), 0.5);
    let serie_p95 = percentil(serie.clone(), 0.95);
    let variante_p50 = percentil(variante.clone(), 0.5);
    let variante_p95 = percentil(variante.clone(), 0.95);

    println!(
        "DLX-04 transferencia DASH (init {} bytes + {} segmentos, delay {:?}, {} corridas): \
         serie p50={}ms p95={}ms | variante p50={}ms p95={}ms | techo recuperable ≈ {}ms/pista",
        init_len,
        seg_len,
        DELAY_POR_SEGMENTO,
        CORRIDAS,
        serie_p50,
        serie_p95,
        variante_p50,
        variante_p95,
        (seg_len as u128 - 1) * DELAY_POR_SEGMENTO.as_millis(),
    );

    // Cota inferior determinista de la serie: el servidor retrasa cada
    // segmento D y el bucle los espera uno a uno (el init no lleva retardo).
    let cota_serie = (seg_len as u128) * DELAY_POR_SEGMENTO.as_millis();
    for d in &serie {
        assert!(
            *d >= cota_serie,
            "la serie no puede durar menos que N·D ({d} < {cota_serie})"
        );
    }
    // La variante paga una sola latencia (todas las peticiones en vuelo a la
    // vez): cota superior holgada 2·D más holgura de planificación.
    let cota_variante = 2 * DELAY_POR_SEGMENTO.as_millis() + 500;
    for d in &variante {
        assert!(
            *d < cota_variante,
            "la variante concurrente no debe serializar ({d} >= {cota_variante})"
        );
    }
}
