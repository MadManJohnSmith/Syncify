//! DLX-04 (#45): fixture DASH contra servidor HTTP local.
//!
//! El brazo Tidal debe pedir y consumir los segmentos DASH en orden, sin
//! saltos, y validar el fichero sólo con el payload completo en disco. El
//! fixture es el fMP4-FLAC real de `syncify-fmp4-flac` (init + 4 fragmentos
//! moof+mdat), servido como manifiesto `DASH_MANIFEST|init|seg_$Number$|N`.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use syncify_fmp4_flac::convert_fmp4_to_flac;
use syncify_tidal_downloader::TidalDownloader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const FIXTURE: &str = "../syncify-fmp4-flac/tests/fixtures/tiny-fmp4-flac.m4a";

/// Divide el fixture en (init, segmentos moof+mdat), igual que el test del
/// crate propietario del fixture.
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

struct Servidor {
    base: String,
    handle: tokio::task::JoinHandle<()>,
    /// Rutas servidas en el orden exacto en que llegaron.
    orden: Arc<Mutex<Vec<String>>>,
}

/// Sirve el fixture como manifiesto DASH. `segmento_ausente` responde 404
/// para ese número de segmento: reproduce el salto de un segmento que nunca
/// llega en mitad de la transferencia.
async fn arrancar_servidor(segmento_ausente: Option<usize>) -> Servidor {
    let data = std::fs::read(fixture_path()).expect("fixture fMP4 presente");
    let (init, segmentos) = split_segments(&data);
    assert_eq!(segmentos.len(), 4, "el fixture trae 4 fragmentos");

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let orden = Arc::new(Mutex::new(Vec::new()));
    let orden_task = orden.clone();
    let handle = tokio::spawn(async move {
        for _ in 0..128 {
            let Ok(Ok((mut socket, _))) =
                tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept()).await
            else {
                break;
            };
            let init = init.clone();
            let segmentos = segmentos.clone();
            let orden = orden_task.clone();
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

                let numero = ruta
                    .rsplit("/seg_")
                    .next()
                    .and_then(|s| s.parse::<usize>().ok());
                let payload = if ruta.ends_with("/init") {
                    Some(init.clone())
                } else if numero == segmento_ausente {
                    None
                } else {
                    numero
                        .and_then(|n| n.checked_sub(1))
                        .and_then(|idx| segmentos.get(idx))
                        .cloned()
                };
                let respuesta = match &payload {
                    Some(bytes) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    )
                    .into_bytes(),
                    None => {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_vec()
                    }
                };
                if socket.write_all(&respuesta).await.is_err() {
                    return;
                }
                if let Some(bytes) = payload {
                    let _ = socket.write_all(&bytes).await;
                }
            });
        }
    });
    Servidor {
        base,
        handle,
        orden,
    }
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

fn spec_dash(base: &str, total: usize) -> String {
    format!("DASH_MANIFEST|{base}/init|{base}/seg_$Number$|{total}")
}

fn orden_esperada(total: usize) -> Vec<String> {
    std::iter::once("/init".to_string())
        .chain((1..=total).map(|n| format!("/seg_{n}")))
        .collect()
}

/// El caso del issue: los segmentos se piden y consumen en orden, sin
/// saltos, y el FLAC entregado es byte a byte el de la conversión offline de
/// referencia (la calidad entregada no cambia).
#[tokio::test]
async fn segmentos_en_orden_y_flac_identico_a_la_referencia() {
    let server = arrancar_servidor(None).await;
    let dir = tempfile::tempdir().unwrap();
    let salida = dir.path().join("track.flac");

    let bytes = TidalDownloader::new()
        .download_audio_payload_with_progress(&spec_dash(&server.base, 4), &salida, |_, _, _| {})
        .await
        .expect("descarga DASH completa");

    let orden = server.orden.lock().unwrap().clone();
    assert_eq!(
        orden,
        orden_esperada(4),
        "se piden init y los 4 segmentos en orden, sin saltos ni duplicados"
    );
    assert!(bytes > 0);

    // El temporal consumió el payload completo y desapareció al promocionar.
    assert!(
        !salida.with_extension("stream.tmp").exists(),
        "el temporal se renombra, no queda parcial"
    );

    // Identidad de la entrega: el FLAC nativo remuxado es exactamente el que
    // produce la conversión offline del fixture concatenado.
    let data = std::fs::read(fixture_path()).unwrap();
    let (init, segmentos) = split_segments(&data);
    let concatenado = dir.path().join("referencia-fmp4.bin");
    let mut ref_file = std::fs::File::create(&concatenado).unwrap();
    ref_file.write_all(&init).unwrap();
    for seg in &segmentos {
        ref_file.write_all(seg).unwrap();
    }
    ref_file.flush().unwrap();
    let referencia = dir.path().join("referencia.flac");
    let muestras = convert_fmp4_to_flac(&concatenado, &referencia).expect("conversión de ref.");
    assert_eq!(muestras, 88_200, "el fixture completo son 2 s a 44,1 kHz");

    let entregado = std::fs::read(&salida).unwrap();
    let esperado = std::fs::read(&referencia).unwrap();
    assert!(
        entregado.starts_with(b"fLaC"),
        "el entregado es FLAC nativo"
    );
    assert_eq!(
        entregado, esperado,
        "el FLAC entregado es idéntico a la conversión de referencia"
    );
    server.handle.abort();
}

/// Un recuento de segmentos malformado o cero se rechaza antes de abrir una
/// sola petición: sin parse estricto, el recuento caía a `unwrap_or(1)` y la
/// descarga se truncaba a un segmento sin aviso.
#[tokio::test]
async fn spec_malformada_se_rechaza_sin_ninguna_peticion() {
    for recuento in ["abc", "0", ""] {
        let server = arrancar_servidor(None).await;
        let dir = tempfile::tempdir().unwrap();
        let salida = dir.path().join("track.flac");

        let spec = format!(
            "DASH_MANIFEST|{}/init|{}/seg_$Number$|{}",
            server.base, server.base, recuento
        );
        let res = TidalDownloader::new()
            .download_audio_payload_with_progress(&spec, &salida, |_, _, _| {})
            .await;

        let err = res
            .err()
            .unwrap_or_else(|| panic!("recuento {recuento:?} debe fallar"));
        assert!(
            err.to_string().contains("InvalidDashSpec"),
            "recuento {recuento:?} debe dar InvalidDashSpec, fue: {err}"
        );
        assert!(
            server.orden.lock().unwrap().is_empty(),
            "ninguna petición sale antes de validar la spec"
        );
        assert!(!salida.exists());
        assert!(!salida.with_extension("stream.tmp").exists());
        server.handle.abort();
    }
}

/// El salto se reproduce: el segmento 2 nunca llega. Tras agotar los
/// reintentos la descarga falla con `SegmentDownloadFailed`, no queda fichero
/// parcial ni promovido, y el orden pedido sigue siendo estricto.
#[tokio::test]
async fn salto_de_segmento_no_promociona_parcial() {
    let server = arrancar_servidor(Some(2)).await;
    let dir = tempfile::tempdir().unwrap();
    let salida = dir.path().join("track.flac");

    let res = TidalDownloader::new()
        .download_audio_payload_with_progress(&spec_dash(&server.base, 4), &salida, |_, _, _| {})
        .await;

    let err = res.expect_err("el salto del segmento 2 debe tumbar la descarga");
    assert!(
        err.to_string().contains("SegmentDownloadFailed"),
        "fallo de segmento clasificado, fue: {err}"
    );
    let orden = server.orden.lock().unwrap().clone();
    let esperado = vec![
        "/init".to_string(),
        "/seg_1".to_string(),
        "/seg_2".to_string(),
        "/seg_2".to_string(),
        "/seg_2".to_string(),
    ];
    assert_eq!(
        orden, esperado,
        "se pide init, seg_1 y los tres reintentos de seg_2, nunca seg_3/seg_4"
    );
    assert!(!salida.exists(), "no se promociona un payload incompleto");
    assert!(
        !salida.with_extension("stream.tmp").exists(),
        "el parcial se purga"
    );
    server.handle.abort();
}

/// El manifiesto puede declarar menos segmentos que los que el servidor
/// tiene: la spec manda, se consumen exactamente los declarados y el orden
/// sigue siendo estricto (sin saltos dentro de lo declarado).
#[tokio::test]
async fn recuento_declarado_manda_y_el_orden_se_cumple() {
    let server = arrancar_servidor(None).await;
    let dir = tempfile::tempdir().unwrap();
    let salida = dir.path().join("track.flac");

    TidalDownloader::new()
        .download_audio_payload_with_progress(&spec_dash(&server.base, 2), &salida, |_, _, _| {})
        .await
        .expect("descarga con 2 segmentos declarados");

    let orden = server.orden.lock().unwrap().clone();
    assert_eq!(
        orden,
        orden_esperada(2),
        "exactamente init + seg_1 + seg_2, en orden"
    );
    server.handle.abort();
}

/// Guard de limpieza: nada de las pruebas de este fichero deja temporales
/// fuera de los `tempdir` desechables (las descargas que fallan purgan su
/// `.stream.tmp`).
#[tokio::test]
async fn fallo_en_init_no_deja_temporal() {
    // Servidor que no existe: el init agota reintentos y falla limpio.
    let dir = tempfile::tempdir().unwrap();
    let salida = dir.path().join("track.flac");
    let spec = "DASH_MANIFEST|http://127.0.0.1:9/init.mp4|http://127.0.0.1:9/seg_$Number$.mp4|4";
    let res = TidalDownloader::new()
        .download_audio_payload_with_progress(spec, &salida, |_, _, _| {})
        .await;
    let err = res.expect_err("sin servidor debe fallar");
    assert!(err.to_string().contains("SegmentDownloadFailed"));
    assert!(!salida.exists());
    assert!(!salida.with_extension("stream.tmp").exists());
}
