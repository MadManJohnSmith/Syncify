//! DLX-01 isolated transport/phase baseline, not a full provider or tagging pipeline.
//! Run with HOME/TMPDIR/XDG_* pointing to a disposable directory outside the personal profile.
use std::time::{Duration, Instant};
use syncify_tauri_lib::download::progress::{
    DownloadPhase, DownloadPhaseRecord, DownloadPhaseTracker,
};
use syncify_tauri_lib::download::qobuz::QobuzDownloader;
use syncify_tauri_lib::download::timeline::{
    compare_and_report, persist_artifact, record_download_result, TimelineArtifact,
    CATEGORIA_COMPLEMENTOS,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUDIO: &[u8] = include_bytes!("fixtures/silence.flac");
const COVER: &[u8] = b"synthetic-cover";
const LYRICS: &[u8] = b"[00:00.00]synthetic";

async fn serve(
    listener: TcpListener,
    max_conns: usize,
    audio_delay: Duration,
    aux_delay: Duration,
) {
    for _ in 0..max_conns {
        let Ok(Ok((socket, _))) =
            tokio::time::timeout(Duration::from_secs(10), listener.accept()).await
        else {
            break;
        };
        // Cada conexión se atiende en su tarea: un retardo de portada no
        // serializa al audio (en la corrida solapada piden a la vez).
        tokio::spawn(atender(socket, audio_delay, aux_delay));
    }
}

async fn atender(mut socket: tokio::net::TcpStream, audio_delay: Duration, aux_delay: Duration) {
    let mut request = [0u8; 2048];
    let Ok(n) = socket.read(&mut request).await else {
        return;
    };
    let payload = if request[..n].starts_with(b"GET /audio ") {
        AUDIO
    } else if request[..n].starts_with(b"GET /cover ") {
        COVER
    } else {
        LYRICS
    };
    let delay = if request[..n].starts_with(b"GET /audio ") {
        audio_delay
    } else {
        aux_delay
    };
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    if socket.write_all(header.as_bytes()).await.is_err() {
        return;
    }
    let _ = socket.write_all(payload).await;
}

#[tokio::test]
async fn local_audio_and_auxiliary_timeline_two_runs() {
    let root = tempfile::tempdir().expect("isolated TMPDIR required");
    let client = reqwest::Client::new();
    for run in 0..2 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let server = tokio::spawn(serve(
            listener,
            3,
            Duration::ZERO,
            Duration::from_millis(20),
        ));
        let mut tracker = DownloadPhaseTracker::new();
        let start = Instant::now();
        tracker.start_phase(DownloadPhase::ResolveStream);
        // The local stream address is known; this is a no-op resolution control.
        tracker.start_phase(DownloadPhase::Transfer);
        let staging = root.path().join(format!("run-{run}.part"));
        let bytes = tokio::time::timeout(
            Duration::from_secs(10),
            QobuzDownloader::new().download_to_staging(
                &format!("{base}/audio"),
                &staging,
                "synthetic",
            ),
        )
        .await
        .expect("transport timeout")
        .expect("transport failed");
        assert_eq!(bytes as usize, AUDIO.len());
        assert_eq!(tokio::fs::read(&staging).await.unwrap(), AUDIO);
        tracker.set_transfer_metrics(bytes, "network");
        tracker.start_phase(DownloadPhase::ValidateAudio);
        assert!(AUDIO.starts_with(b"fLaC"));
        tracker.start_phase(DownloadPhase::ResolveCover);
        let cover = client
            .get(format!("{base}/cover"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(cover.as_ref(), COVER);
        tracker.start_phase(DownloadPhase::ResolveLyrics);
        let lyrics = client
            .get(format!("{base}/lyrics"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(lyrics.as_ref(), LYRICS);
        tracker.start_phase(DownloadPhase::EnrichMetadata);
        tracker.start_phase(DownloadPhase::Tagging);
        tracker.start_phase(DownloadPhase::Promotion);
        tracker.start_phase(DownloadPhase::Persisting);
        let timings = tracker.finish_completed();
        server.await.unwrap();
        assert_eq!(timings.bytes_transferred, bytes);
        assert!(timings.transfer_ms > 0);
        assert!(timings.total_duration_ms <= start.elapsed().as_millis() as u64 + 1);
        assert!(timings
            .phases
            .windows(2)
            .all(|w| w[0].end_ms <= w[1].start_ms));
        assert!(timings.phases.iter().all(|p| p.end_ms >= p.start_ms));
        // Aggregate is rounded to milliseconds; allow 1ms per phase for quantization.
        let sum: u64 = timings.phases.iter().map(|p| p.duration_ms).sum();
        assert!(sum <= timings.total_duration_ms + timings.phases.len() as u64);
        println!("local_baseline run={} audio_bytes={} transfer_ms={} cover_ms={} lyrics_ms={} total_ms={} phase_sum_ms={}", run + 1, bytes, timings.transfer_ms, timings.cover_duration_ms, timings.lyrics_duration_ms, timings.total_duration_ms, sum);
    }
}

/// Recorre las etapas del pipeline en el orden de producción (cola DLX §2)
/// contra un servidor local: transferencia real a disco, validación del
/// payload, portada y letras por red local, promoción con rename real y
/// persistencia con SQLite `:memory:`. Devuelve la cronología completa.
async fn correr_pipeline_local(
    root: &std::path::Path,
    run: usize,
    corrida: &str,
    audio_delay: Duration,
    aux_delay: Duration,
) -> TimelineArtifact {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let server = tokio::spawn(serve(listener, 3, audio_delay, aux_delay));
    let client = reqwest::Client::new();

    let mut tracker = DownloadPhaseTracker::new();
    // 1-2. Auth y resolución: con servidor local la cuenta y la URL son
    // conocidas; fases de control, igual que en el baseline de transporte.
    tracker.start_phase(DownloadPhase::Auth);
    tracker.start_phase(DownloadPhase::ResolveStream);

    // 3. Transferencia real a fichero temporal (disco + red local).
    tracker.start_phase(DownloadPhase::Transfer);
    let staging = root.join(format!("cronologia-{run}.part"));
    let bytes = tokio::time::timeout(
        Duration::from_secs(10),
        QobuzDownloader::new().download_to_staging(&format!("{base}/audio"), &staging, "synthetic"),
    )
    .await
    .expect("transport timeout")
    .expect("transport failed");
    tracker.set_transfer_metrics(bytes, "network");

    // 4. Validación del payload: relectura del temporal (disco + CPU).
    tracker.start_phase(DownloadPhase::ValidateAudio);
    let staged = tokio::fs::read(&staging).await.unwrap();
    assert_eq!(staged, AUDIO);
    assert!(staged.starts_with(b"fLaC"));

    // 5-6. Portada y letras por la red de prueba.
    tracker.start_phase(DownloadPhase::ResolveCover);
    let cover = client
        .get(format!("{base}/cover"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(cover.as_ref(), COVER);
    tracker.start_phase(DownloadPhase::ResolveLyrics);
    let lyrics = client
        .get(format!("{base}/lyrics"))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(lyrics.as_ref(), LYRICS);

    // 7-8. Enriquecimiento (control sin red) y etiquetado (control).
    tracker.start_phase(DownloadPhase::EnrichMetadata);
    tracker.start_phase(DownloadPhase::Tagging);

    // 9. Promoción: rename atómico a la ruta final, como en producción.
    tracker.start_phase(DownloadPhase::Promotion);
    let final_path = root.join(format!("cronologia-{run}.flac"));
    tokio::fs::rename(&staging, &final_path).await.unwrap();
    assert!(final_path.exists());

    // 10. Persistencia: escritura real en SQLite :memory: (la espera de SQLite
    // queda dentro de la etapa Persisting, separada de red y disco).
    tracker.start_phase(DownloadPhase::Persisting);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(":memory:")
        .await
        .expect("sqlite :memory:");
    sqlx::query("CREATE TABLE libro_mayor_dlx01 (id INTEGER PRIMARY KEY, bytes INTEGER NOT NULL)")
        .execute(&pool)
        .await
        .expect("crea tabla");
    sqlx::query("INSERT INTO libro_mayor_dlx01 (bytes) VALUES (?)")
        .bind(bytes as i64)
        .execute(&pool)
        .await
        .expect("inserta");

    let timings = tracker.finish_completed();
    server.await.unwrap();
    TimelineArtifact::from_timings(corrida, "qobuz", &timings)
}

const ETAPAS_ESPERADAS: &[&str] = &[
    "Auth",
    "ResolveStream",
    "Transfer",
    "ValidateAudio",
    "ResolveCover",
    "ResolveLyrics",
    "EnrichMetadata",
    "Tagging",
    "Promotion",
    "Persisting",
];

fn assert_cronologia_valida_y_completa(artifact: &TimelineArtifact) {
    let fases: Vec<&str> = artifact.etapas.iter().map(|r| r.phase.as_str()).collect();
    for etapa in ETAPAS_ESPERADAS {
        assert!(
            fases.contains(etapa),
            "cronología incompleta: falta {etapa}; registradas: {fases:?}"
        );
    }
    assert!(artifact.etapas.iter().all(|p| p.end_ms >= p.start_ms));
    assert!(artifact.total_pared_ms > 0);
    // El total se explica: unión más huecos ≈ total pared (cuantización 1 ms
    // por etapa).
    assert!(
        artifact.union_ms + artifact.huecos_ms
            <= artifact.total_pared_ms + artifact.etapas.len() as u64,
        "unión {} + huecos {} debe cuadrar con total {}",
        artifact.union_ms,
        artifact.huecos_ms,
        artifact.total_pared_ms
    );
    // En el pipeline serial la unión cubre (casi) todo: sin solape la suma
    // equivale a la unión con la misma tolerancia de cuantización.
    let suma: u64 = artifact.etapas.iter().map(|p| p.duration_ms).sum();
    assert!(suma <= artifact.union_ms + artifact.etapas.len() as u64);
    // Fracción de audio: la transferencia debe ser visible y distinta de cero.
    let audio = artifact
        .categorias
        .iter()
        .find(|c| c.categoria == "audio")
        .expect("categoría audio");
    assert!(audio.union_ms > 0);
    assert!(audio.fraccion > 0.0);
}

/// Igualdad de cronologías tolerante con los f64: el serializador JSON no
/// conserva todos los bits al pasar por texto decimal, así que el roundtrip
/// se compara campo a campo con épsilon para las fracciones.
fn assert_cronologias_equivalentes(a: &TimelineArtifact, b: &TimelineArtifact) {
    assert_eq!(a.schema_version, b.schema_version);
    assert_eq!(a.corrida, b.corrida);
    assert_eq!(a.servicio, b.servicio);
    assert_eq!(a.registrada_en, b.registrada_en);
    assert_eq!(a.total_pared_ms, b.total_pared_ms);
    assert_eq!(a.union_ms, b.union_ms);
    assert_eq!(a.suma_etapas_ms, b.suma_etapas_ms);
    assert_eq!(a.huecos_ms, b.huecos_ms);
    assert_eq!(
        a.transferencia.bytes_transferred,
        b.transferencia.bytes_transferred
    );
    assert_eq!(
        a.transferencia.transfer_source,
        b.transferencia.transfer_source
    );
    assert!((a.transferencia.throughput_mibps - b.transferencia.throughput_mibps).abs() < 1e-9);
    assert_eq!(a.categorias.len(), b.categorias.len());
    for (ca, cb) in a.categorias.iter().zip(b.categorias.iter()) {
        assert_eq!(ca.categoria, cb.categoria);
        assert_eq!(ca.union_ms, cb.union_ms);
        assert_eq!(ca.suma_ms, cb.suma_ms);
        assert!((ca.fraccion - cb.fraccion).abs() < 1e-9);
    }
    assert_eq!(a.etapas, b.etapas);
}

#[tokio::test]
async fn full_pipeline_chronology_is_valid_complete_and_persisted() {
    let root = tempfile::tempdir().expect("isolated TMPDIR required");
    let out_dir = root.path().join("cronologias");

    let baseline = correr_pipeline_local(
        root.path(),
        1,
        "baseline",
        Duration::ZERO,
        Duration::from_millis(20),
    )
    .await;
    assert_cronologia_valida_y_completa(&baseline);

    // Criterio 3: el informe explica el total con sus etapas y esperas.
    let informe = baseline.report();
    assert!(informe.contains("total pared:"), "{informe}");
    assert!(informe.contains("unión de intervalos:"), "{informe}");
    assert!(informe.contains("huecos sin atribuir:"), "{informe}");
    assert!(informe.contains("audio"), "{informe}");
    assert!(informe.contains("solapables"), "{informe}");

    // Criterio 2: el artefacto no guarda URLs, credenciales ni datos privados.
    let segunda = correr_pipeline_local(
        root.path(),
        2,
        "repeticion",
        Duration::ZERO,
        Duration::from_millis(20),
    )
    .await;
    assert_cronologia_valida_y_completa(&segunda);
    let json = serde_json::to_string(&segunda).expect("serializa");
    assert!(
        !json.contains("http"),
        "el artefacto no debe contener URLs: {json}"
    );

    // Criterio 4: la comparación entre las dos corridas se emite por unión.
    let comparacion = compare_and_report(&baseline, &segunda);
    assert!(comparacion.contains("unión de intervalos"), "{comparacion}");

    // La cronología persiste y el fichero hace roundtrip idéntico.
    for artifact in [&baseline, &segunda] {
        let ruta = persist_artifact(&out_dir, artifact).expect("persiste");
        assert!(ruta.extension().is_some_and(|e| e == "json"));
        let leido: TimelineArtifact =
            serde_json::from_str(&std::fs::read_to_string(&ruta).expect("lee artefacto"))
                .expect("deserializa");
        assert_cronologias_equivalentes(&leido, artifact);
    }
    assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 2);
}

#[tokio::test]
async fn two_runs_comparison_uses_union_of_intervals_with_real_overlap() {
    let root = tempfile::tempdir().expect("isolated TMPDIR required");
    // Red de prueba realista: audio lento (500 ms) y complementos lentos
    // (200 ms cada uno). La espera de portada y letras es la evitable que
    // DLX-02 solaparía con la transferencia.
    let baseline = correr_pipeline_local(
        root.path(),
        3,
        "baseline",
        Duration::from_millis(500),
        Duration::from_millis(200),
    )
    .await;
    assert_cronologia_valida_y_completa(&baseline);

    // Corrida solapada: portada y letras DURANTE la transferencia. El tracker
    // solo registra fases secuenciales, así que los intervalos reales de los
    // complementos se miden contra el origen del tracker y se incorporan a
    // `phases`; es exactamente lo que DLX-02 hará desde el pipeline.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let server = tokio::spawn(serve(
        listener,
        3,
        Duration::from_millis(500),
        Duration::from_millis(200),
    ));
    let client = reqwest::Client::new();

    let mut tracker = DownloadPhaseTracker::new();
    let origin = tracker.origin_instant;
    tracker.start_phase(DownloadPhase::Auth);
    tracker.start_phase(DownloadPhase::ResolveStream);
    tracker.start_phase(DownloadPhase::Transfer);
    let staging = root.path().join("solapada.part");
    let audio_url = format!("{base}/audio");
    let downloader = QobuzDownloader::new();
    let transfer_fut = downloader.download_to_staging(&audio_url, &staging, "synthetic");
    let aux_fut = async {
        let cover_start = origin.elapsed();
        let cover = client
            .get(format!("{base}/cover"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let lyrics_start = origin.elapsed();
        let lyrics = client
            .get(format!("{base}/lyrics"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        let aux_end = origin.elapsed();
        (cover, lyrics, cover_start, lyrics_start, aux_end)
    };
    let (bytes, (cover, lyrics, cover_start, lyrics_start, aux_end)) =
        tokio::join!(transfer_fut, aux_fut);
    let bytes = bytes.expect("transport failed");
    assert_eq!(cover.as_ref(), COVER);
    assert_eq!(lyrics.as_ref(), LYRICS);
    tracker.set_transfer_metrics(bytes, "network");

    tracker.start_phase(DownloadPhase::ValidateAudio);
    assert!(tokio::fs::read(&staging)
        .await
        .unwrap()
        .starts_with(b"fLaC"));
    tracker.start_phase(DownloadPhase::EnrichMetadata);
    tracker.start_phase(DownloadPhase::Tagging);
    tracker.start_phase(DownloadPhase::Promotion);
    tokio::fs::rename(&staging, root.path().join("solapada.flac"))
        .await
        .unwrap();
    tracker.start_phase(DownloadPhase::Persisting);
    let mut timings = tracker.finish_completed();
    server.await.unwrap();

    // Intervalos reales medidos de los complementos solapados.
    let a_ms = |d: Duration| d.as_millis() as u64;
    timings.phases.push(DownloadPhaseRecord {
        phase: DownloadPhase::ResolveCover,
        start_ms: a_ms(cover_start),
        end_ms: a_ms(lyrics_start),
        duration_ms: a_ms(lyrics_start.saturating_sub(cover_start)),
    });
    timings.phases.push(DownloadPhaseRecord {
        phase: DownloadPhase::ResolveLyrics,
        start_ms: a_ms(lyrics_start),
        end_ms: a_ms(aux_end),
        duration_ms: a_ms(aux_end.saturating_sub(lyrics_start)),
    });
    let solapada = TimelineArtifact::from_timings("solapada", "qobuz", &timings);

    // Ambas corridas hicieron el mismo trabajo: sus sumas son del mismo
    // orden. La igualdad exacta no se afirma a propósito: las sumas arrastran
    // jitters absolutos de carga que no se cancelan entre corridas, y la
    // semántica exacta de «suma vs unión» ya la fija el test unitario de
    // `timeline.rs` con intervalos sintéticos deterministas.
    let delta_suma = solapada.suma_etapas_ms as i64 - baseline.suma_etapas_ms as i64;
    println!("delta de sumas entre corridas (informativo): {delta_suma} ms");
    // Por la unión, la espera de complementos desaparece del tiempo total.
    let complementos_baseline = baseline
        .categorias
        .iter()
        .find(|c| c.categoria == CATEGORIA_COMPLEMENTOS)
        .unwrap()
        .union_ms;
    assert!(
        complementos_baseline > 150,
        "espera serial de complementos: {complementos_baseline}"
    );
    // En el baseline serial, unión y suma coinciden salvo la cuantización a
    // ms (un intervalo puede perder 1 ms de duración por truncado).
    assert!(
        baseline.union_ms <= baseline.suma_etapas_ms + baseline.etapas.len() as u64,
        "baseline serial: unión {} vs suma {}",
        baseline.union_ms,
        baseline.suma_etapas_ms
    );
    assert!(
        solapada.suma_etapas_ms > solapada.union_ms + 50,
        "la corrida solapada debe tener suma > unión (trabajo en paralelo): suma {} unión {}",
        solapada.suma_etapas_ms,
        solapada.union_ms
    );

    let cmp = baseline.compare_runs(&solapada);
    assert!(
        cmp.delta_total_ms < -50,
        "la unión debe reflejar el solapamiento como mejora: {:?}",
        cmp
    );
    // El trabajo de complementos es el MISMO en ambas corridas: su unión
    // propia no cambia (categorías honestas); lo que cambia es que ya no
    // extienden el tiempo total. El delta real lo captura el total por unión.
    let complementos_delta = cmp
        .categorias
        .iter()
        .find(|c| c.categoria == CATEGORIA_COMPLEMENTOS)
        .unwrap()
        .delta_ms;
    assert!(
        complementos_delta.abs() < 150,
        "delta de complementos debería ser ~0 (mismo trabajo): {complementos_delta}"
    );
    // El informe de la corrida solapada debe señalar trabajo contado doble
    // (suma > unión), que es lo que la comparación por unión evita.
    let informe_candidata = solapada.report();
    assert!(
        informe_candidata.contains("contaría doble"),
        "{informe_candidata}"
    );
    let informe = compare_and_report(&baseline, &solapada);
    assert!(
        informe.contains("unión de intervalos, no sobre la suma"),
        "{informe}"
    );
    println!("{informe}");
}

/// El hook del orquestador es opt-in: sin env no escribe nada; con env deja
/// el artefacto en el directorio configurado.
#[tokio::test]
async fn record_download_result_is_opt_in_and_sanitized() {
    let root = tempfile::tempdir().expect("tmpdir");
    let dir = root.path().join("cronologias");

    // Sin env: no escribe nada y no falla.
    std::env::remove_var(syncify_tauri_lib::download::timeline::TIMELINE_DIR_ENV);
    let resultado_sin_env = make_result(Some(50));
    record_download_result(&resultado_sin_env);
    assert!(!dir.exists());

    // Con env: un fichero por resultado, sanitizado.
    std::env::set_var(
        syncify_tauri_lib::download::timeline::TIMELINE_DIR_ENV,
        &dir,
    );
    std::env::set_var(
        syncify_tauri_lib::download::timeline::RUN_LABEL_ENV,
        "hook-test",
    );
    record_download_result(&make_result(Some(50)));
    let ficheros: Vec<String> = std::fs::read_dir(&dir)
        .expect("directorio creado")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(ficheros.len(), 1, "{ficheros:?}");
    assert!(
        ficheros[0].starts_with("dlx-cronologia-hook-test-qobuz-"),
        "{ficheros:?}"
    );
    let cuerpo = std::fs::read_to_string(dir.join(&ficheros[0])).unwrap();
    assert!(!cuerpo.contains("http"));

    // Resultado sin cronología (p. ej. ruta legacy): no escribe nada.
    let antes = std::fs::read_dir(&dir).unwrap().count();
    record_download_result(&make_result(None));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), antes);

    std::env::remove_var(syncify_tauri_lib::download::timeline::TIMELINE_DIR_ENV);
    std::env::remove_var(syncify_tauri_lib::download::timeline::RUN_LABEL_ENV);
}

fn make_result(transfer_ms: Option<u64>) -> syncify_tauri_lib::download::progress::DownloadResult {
    use syncify_tauri_lib::download::progress::{DownloadPhaseTimings, DownloadResult};
    let phase_timings = transfer_ms.map(|ms| DownloadPhaseTimings {
        total_duration_ms: ms,
        transfer_ms: ms,
        stream_duration_ms: ms,
        bytes_transferred: 1024,
        transfer_source: "network".to_string(),
        throughput_mibps: 1.0,
        ..Default::default()
    });
    DownloadResult {
        service: "qobuz".to_string(),
        // Datos privados que el artefacto NO debe copiar.
        title: "Pista Privada".to_string(),
        artist: "Artista Privado".to_string(),
        isrc: Some("ESX000000001".to_string()),
        file_path: "/home/privado/musica/pista.flac".to_string(),
        phase_timings,
        ..Default::default()
    }
}
