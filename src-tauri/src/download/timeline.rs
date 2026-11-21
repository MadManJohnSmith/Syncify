//! DLX-01: cronología por pista como artefacto de medición (issue #42).
//!
//! Ambos pipelines (Qobuz y Tidal) ya llevan un `DownloadPhaseTracker` que
//! anota el inicio y fin de cada etapa. Este módulo convierte esas mediciones
//! en un artefacto JSON de medición, sin esquema nuevo en la BD: un fichero
//! por descarga en el directorio configurable con `SYNCIFY_DLX_TIMELINE_DIR`.
//! Sin esa variable el hook del orquestador no escribe nada y el resultado de
//! la descarga queda idéntico.
//!
//! El artefacto no guarda credenciales, URLs firmadas ni datos privados:
//! solo nombres de etapa, milisegundos, bytes transferidos y la etiqueta de
//! corrida que pase el operador (sanitizada para el nombre de fichero). No
//! copia título, artista, ISRC, rutas ni identificadores del resultado.
//!
//! Informe y comparación: el tiempo total se explica como unión de intervalos
//! más huecos sin atribuir; la comparación entre corridas usa la unión de
//! intervalos de cada categoría, nunca la suma (que contaría dos veces el
//! trabajo solapado que introduzca DLX-02).
//!
//! Limitación conocida: el hook del orquestador solo registra descargas que
//! terminan en `Ok`; la cronología de intentos fallidos exigiría tocar las
//! ramas de error de los pipelines y DLX-01 no las necesita.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::progress::{DownloadPhase, DownloadPhaseRecord, DownloadPhaseTimings, DownloadResult};

/// Directorio donde se escriben los artefactos de cronología.
pub const TIMELINE_DIR_ENV: &str = "SYNCIFY_DLX_TIMELINE_DIR";

/// Etiqueta de corrida opcional (p. ej. `baseline` o `solapada`).
pub const RUN_LABEL_ENV: &str = "SYNCIFY_DLX_RUN_LABEL";

/// Etiqueta de corrida por defecto cuando no se fija `RUN_LABEL_ENV`.
pub const CORRIDA_POR_DEFECTO: &str = "app";

pub const SCHEMA_VERSION: u32 = 1;

pub const CATEGORIA_AUDIO: &str = "audio";
pub const CATEGORIA_COMPLEMENTOS: &str = "complementos";
pub const CATEGORIA_PIPELINE: &str = "pipeline";

/// Categoría de medición de una etapa.
///
/// `audio` es la transferencia de bytes; `complementos` son las etapas que no
/// dependen del audio (portada, letras, enriquecimiento), candidatas a
/// solapamiento en DLX-02; `pipeline` es el resto del trabajo serializable
/// (auth, resolución, validación, etiquetado, promoción, persistencia y la
/// espera previa en cola). Los marcadores terminales no se categorizan.
fn categoria_de_fase(phase: DownloadPhase) -> Option<&'static str> {
    match phase {
        DownloadPhase::Transfer => Some(CATEGORIA_AUDIO),
        DownloadPhase::ResolveCover
        | DownloadPhase::ResolveLyrics
        | DownloadPhase::EnrichMetadata => Some(CATEGORIA_COMPLEMENTOS),
        DownloadPhase::QueueWait
        | DownloadPhase::Auth
        | DownloadPhase::ResolveStream
        | DownloadPhase::ValidateAudio
        | DownloadPhase::Tagging
        | DownloadPhase::Promotion
        | DownloadPhase::Persisting => Some(CATEGORIA_PIPELINE),
        DownloadPhase::Completed | DownloadPhase::Failed | DownloadPhase::Cancelled => None,
    }
}

/// Une intervalos solapados o contiguos y devuelve la cobertura total en ms.
///
/// Es la métrica canónica para comparar corridas: la suma de duraciones
/// contaría dos veces el trabajo que DLX-02 solape, la unión no.
fn union_intervalos(intervalos: &[(u64, u64)]) -> u64 {
    let mut ordenados: Vec<(u64, u64)> = intervalos.to_vec();
    ordenados.sort_unstable();
    let mut union: u64 = 0;
    let mut actual: Option<(u64, u64)> = None;
    for (inicio, fin) in ordenados {
        match actual {
            Some((a, b)) if inicio <= b => {
                actual = Some((a, b.max(fin)));
            }
            Some((a, b)) => {
                union += b.saturating_sub(a);
                actual = Some((inicio, fin));
            }
            None => actual = Some((inicio, fin)),
        }
    }
    if let Some((a, b)) = actual {
        union += b.saturating_sub(a);
    }
    union
}

fn union_de_etapas(etapas: &[DownloadPhaseRecord]) -> u64 {
    let intervalos: Vec<(u64, u64)> = etapas.iter().map(|r| (r.start_ms, r.end_ms)).collect();
    union_intervalos(&intervalos)
}

fn suma_de_etapas(etapas: &[DownloadPhaseRecord]) -> u64 {
    etapas.iter().map(|r| r.duration_ms).sum()
}

/// Resumen de la transferencia de bytes de audio.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransferSummary {
    pub bytes_transferred: u64,
    pub transfer_source: String,
    pub throughput_mibps: f64,
}

/// Cobertura de una categoría de medición sobre la cronología.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryUsage {
    pub categoria: String,
    pub union_ms: u64,
    pub suma_ms: u64,
    /// Unión de la categoría dividida por el total pared (o por la unión si
    /// el total es 0).
    pub fraccion: f64,
}

/// Artefacto de cronología de una descarga, medido por etapas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineArtifact {
    pub schema_version: u32,
    pub corrida: String,
    pub servicio: String,
    /// Instante de escritura en RFC 3339 UTC.
    pub registrada_en: String,
    pub total_pared_ms: u64,
    pub union_ms: u64,
    pub suma_etapas_ms: u64,
    /// Tiempo pared no cubierto por ninguna etapa registrada.
    pub huecos_ms: u64,
    pub transferencia: TransferSummary,
    pub categorias: Vec<CategoryUsage>,
    pub etapas: Vec<DownloadPhaseRecord>,
}

impl TimelineArtifact {
    /// Construye la cronología a partir de las mediciones del pipeline.
    pub fn from_timings(corrida: &str, servicio: &str, timings: &DownloadPhaseTimings) -> Self {
        let etapas = timings.phases.clone();
        let union_ms = union_de_etapas(&etapas);
        let suma_etapas_ms = suma_de_etapas(&etapas);
        let total_pared_ms = timings.total_duration_ms;
        let huecos_ms = total_pared_ms.saturating_sub(union_ms);
        // Denominador de fracciones: el tiempo pared; si no hay total medido,
        // la unión evita dividir por cero.
        let denominador = total_pared_ms.max(union_ms);
        let categorias = [CATEGORIA_AUDIO, CATEGORIA_COMPLEMENTOS, CATEGORIA_PIPELINE]
            .into_iter()
            .map(|categoria| {
                let propias: Vec<DownloadPhaseRecord> = etapas
                    .iter()
                    .filter(|r| categoria_de_fase(r.phase) == Some(categoria))
                    .cloned()
                    .collect();
                let union = union_de_etapas(&propias);
                let fraccion = if denominador > 0 {
                    union as f64 / denominador as f64
                } else {
                    0.0
                };
                CategoryUsage {
                    categoria: categoria.to_string(),
                    union_ms: union,
                    suma_ms: suma_de_etapas(&propias),
                    fraccion,
                }
            })
            .collect();
        Self {
            schema_version: SCHEMA_VERSION,
            corrida: corrida.to_string(),
            servicio: servicio.to_string(),
            registrada_en: chrono::Utc::now().to_rfc3339(),
            total_pared_ms,
            union_ms,
            suma_etapas_ms,
            huecos_ms,
            transferencia: TransferSummary {
                bytes_transferred: timings.bytes_transferred,
                transfer_source: timings.transfer_source.clone(),
                throughput_mibps: timings.throughput_mibps,
            },
            categorias,
            etapas,
        }
    }

    #[allow(dead_code)] // Cubierta por `tests/dlx01_local_baseline_test.rs`.
    fn uso_categoria(&self, categoria: &str) -> u64 {
        self.categorias
            .iter()
            .find(|c| c.categoria == categoria)
            .map(|c| c.union_ms)
            .unwrap_or(0)
    }

    /// Informe textual: el total explicado con sus etapas y esperas.
    #[allow(dead_code)] // Cubierta por `tests/dlx01_local_baseline_test.rs`.
    pub fn report(&self) -> String {
        let denominador = self.total_pared_ms.max(self.union_ms);
        let pct = |ms: u64| -> f64 {
            if denominador > 0 {
                ms as f64 / denominador as f64 * 100.0
            } else {
                0.0
            }
        };
        let mut lineas = Vec::new();
        lineas.push(format!(
            "cronología DLX-01 · corrida={} · servicio={} · registrada={}",
            self.corrida, self.servicio, self.registrada_en
        ));
        lineas.push(format!(
            "total pared: {} ms · unión de intervalos: {} ms ({:.1}%) · huecos sin atribuir: {} ms ({:.1}%)",
            self.total_pared_ms,
            self.union_ms,
            pct(self.union_ms),
            self.huecos_ms,
            pct(self.huecos_ms)
        ));
        if self.suma_etapas_ms > self.union_ms {
            lineas.push(format!(
                "suma de etapas: {} ms — supera la unión: hay trabajo solapado y la suma lo contaría doble",
                self.suma_etapas_ms
            ));
        }
        lineas.push("por categoría (unión):".to_string());
        for c in &self.categorias {
            let papel = if c.categoria == CATEGORIA_AUDIO {
                "transferencia de bytes de audio"
            } else if c.categoria == CATEGORIA_COMPLEMENTOS {
                "portada, letras, enriquecimiento (no dependen del audio; solapables, DLX-02)"
            } else {
                "auth, resolución, validación, etiquetado, promoción, persistencia"
            };
            lineas.push(format!(
                "  {}: {} ms ({:.1}%) — {}",
                c.categoria,
                c.union_ms,
                pct(c.union_ms),
                papel
            ));
        }
        let audio = self.uso_categoria(CATEGORIA_AUDIO);
        let complementos = self.uso_categoria(CATEGORIA_COMPLEMENTOS);
        lineas.push(format!(
            "fracción de audio: {:.1}% · esperas de complementos solapables: {:.1}% · huecos: {:.1}%",
            pct(audio),
            pct(complementos),
            pct(self.huecos_ms)
        ));
        lineas.join("\n")
    }

    /// Compara esta corrida (baseline) con otra (candidata) por unión de
    /// intervalos, nunca por suma de duraciones.
    #[allow(dead_code)] // Cubierta por `tests/dlx01_local_baseline_test.rs`.
    pub fn compare_runs(&self, candidata: &TimelineArtifact) -> RunComparison {
        let delta = |base: u64, cand: u64| cand as i64 - base as i64;
        let categorias = [CATEGORIA_AUDIO, CATEGORIA_COMPLEMENTOS, CATEGORIA_PIPELINE]
            .into_iter()
            .map(|categoria| {
                let base = self.uso_categoria(categoria);
                let cand = candidata.uso_categoria(categoria);
                CategoryDelta {
                    categoria: categoria.to_string(),
                    union_baseline_ms: base,
                    union_candidata_ms: cand,
                    delta_ms: delta(base, cand),
                }
            })
            .collect();
        RunComparison {
            schema_version: SCHEMA_VERSION,
            corrida_baseline: self.corrida.clone(),
            corrida_candidata: candidata.corrida.clone(),
            total_union_baseline_ms: self.union_ms,
            total_union_candidata_ms: candidata.union_ms,
            delta_total_ms: delta(self.union_ms, candidata.union_ms),
            solapamiento_baseline_ms: self.suma_etapas_ms.saturating_sub(self.union_ms),
            solapamiento_candidata_ms: candidata.suma_etapas_ms.saturating_sub(candidata.union_ms),
            categorias,
        }
    }
}

/// Compara dos corridas y devuelve su informe textual combinado.
#[allow(dead_code)] // Cubierta por `tests/dlx01_local_baseline_test.rs`.
pub fn compare_and_report(baseline: &TimelineArtifact, candidata: &TimelineArtifact) -> String {
    let partes = vec![
        baseline.report(),
        candidata.report(),
        baseline.compare_runs(candidata).report(),
    ];
    partes.join("\n---\n")
}

/// Delta de una categoría entre dos corridas, calculado sobre la unión.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)] // Cubierta por `tests/dlx01_local_baseline_test.rs`.
pub struct CategoryDelta {
    pub categoria: String,
    pub union_baseline_ms: u64,
    pub union_candidata_ms: u64,
    pub delta_ms: i64,
}

/// Comparación entre dos corridas por unión de intervalos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)] // Cubierta por `tests/dlx01_local_baseline_test.rs`.
pub struct RunComparison {
    pub schema_version: u32,
    pub corrida_baseline: String,
    pub corrida_candidata: String,
    pub total_union_baseline_ms: u64,
    pub total_union_candidata_ms: u64,
    pub delta_total_ms: i64,
    /// Suma − unión: trabajo contado doble si se comparara por suma.
    pub solapamiento_baseline_ms: u64,
    pub solapamiento_candidata_ms: u64,
    pub categorias: Vec<CategoryDelta>,
}

impl RunComparison {
    /// Informe textual de la comparación.
    #[allow(dead_code)] // Cubierta por `tests/dlx01_local_baseline_test.rs`.
    pub fn report(&self) -> String {
        let mut lineas = Vec::new();
        lineas.push(format!(
            "comparación DLX-01 · baseline={} · candidata={} · deltas sobre la unión de intervalos, no sobre la suma",
            self.corrida_baseline, self.corrida_candidata
        ));
        lineas.push(format!(
            "total (unión): {} ms → {} ms (delta {} ms)",
            self.total_union_baseline_ms, self.total_union_candidata_ms, self.delta_total_ms
        ));
        lineas.push(format!(
            "trabajo solapado (suma − unión): baseline {} ms · candidata {} ms",
            self.solapamiento_baseline_ms, self.solapamiento_candidata_ms
        ));
        for c in &self.categorias {
            lineas.push(format!(
                "  {}: {} ms → {} ms (delta {} ms)",
                c.categoria, c.union_baseline_ms, c.union_candidata_ms, c.delta_ms
            ));
        }
        lineas.join("\n")
    }
}

/// Etiqueta de corrida saneada para el nombre de fichero.
fn sanitize_corrida(corrida: &str) -> String {
    let saneada: String = corrida
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let recortada = saneada.trim_matches('-').to_string();
    if recortada.is_empty() {
        CORRIDA_POR_DEFECTO.to_string()
    } else {
        recortada
    }
}

/// Directorio de cronologías configurado, o `None` si la medición está
/// desactivada (comportamiento por defecto).
pub fn timeline_dir() -> Option<PathBuf> {
    std::env::var(TIMELINE_DIR_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn run_label() -> String {
    std::env::var(RUN_LABEL_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| CORRIDA_POR_DEFECTO.to_string())
}

/// Escribe el artefacto en `dir` y devuelve la ruta del fichero.
pub fn persist_artifact(dir: &Path, artifact: &TimelineArtifact) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let nombre = format!(
        "dlx-cronologia-{}-{}-{}.json",
        sanitize_corrida(&artifact.corrida),
        sanitize_corrida(&artifact.servicio),
        unix_ms
    );
    let destino = dir.join(nombre);
    let temporal = dir.join(format!(
        "{}.tmp",
        destino.file_name().unwrap_or_default().to_string_lossy()
    ));
    let cuerpo = serde_json::to_string_pretty(artifact)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&temporal, cuerpo)?;
    std::fs::rename(&temporal, &destino)?;
    Ok(destino)
}

/// Hook del orquestador: persiste la cronología de una descarga completada.
///
/// Opt-in: sin `SYNCIFY_DLX_TIMELINE_DIR` no hace nada. Un fallo de escritura
/// se registra con `warn` y nunca altera el resultado de la descarga.
pub fn record_download_result(result: &DownloadResult) {
    let Some(dir) = timeline_dir() else {
        return;
    };
    let Some(timings) = result.phase_timings.as_ref() else {
        return;
    };
    let artifact = TimelineArtifact::from_timings(&run_label(), &result.service, timings);
    if let Err(err) = persist_artifact(&dir, &artifact) {
        tracing::warn!(
            error = %err,
            "DLX-01: no se pudo escribir la cronología de descarga"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::progress::CacheHitReport;

    fn record(phase: DownloadPhase, start: u64, end: u64) -> DownloadPhaseRecord {
        DownloadPhaseRecord {
            phase,
            start_ms: start,
            end_ms: end,
            duration_ms: end.saturating_sub(start),
        }
    }

    #[test]
    fn union_fusiona_solapes_y_respeta_huecos() {
        let etapas = vec![
            record(DownloadPhase::Transfer, 10, 30),
            record(DownloadPhase::Transfer, 20, 40),
            record(DownloadPhase::ValidateAudio, 50, 60),
        ];
        assert_eq!(union_de_etapas(&etapas), 40); // [10,40) = 30 + [50,60) = 10
        assert_eq!(suma_de_etapas(&etapas), 50);
    }

    #[test]
    fn from_timings_clasifica_categorias_y_huecos() {
        let timings = DownloadPhaseTimings {
            total_duration_ms: 300,
            bytes_transferred: 1024,
            transfer_source: "network".to_string(),
            throughput_mibps: 1.5,
            phases: vec![
                record(DownloadPhase::Transfer, 0, 100),
                record(DownloadPhase::ResolveCover, 100, 130),
                record(DownloadPhase::ResolveLyrics, 100, 140),
                record(DownloadPhase::Tagging, 140, 190),
                record(DownloadPhase::Persisting, 190, 240),
                record(DownloadPhase::Completed, 240, 240),
            ],
            ..Default::default()
        };
        let artifact = TimelineArtifact::from_timings("baseline", "qobuz", &timings);
        // Unión: [0,240) es contigua (complementos solapados entre sí) = 240;
        // huecos 300-240=60; suma de duraciones 270 (cuenta doble el solape
        // entre portada y letras).
        assert_eq!(artifact.union_ms, 240);
        assert_eq!(artifact.suma_etapas_ms, 270);
        assert_eq!(artifact.huecos_ms, 60);
        assert_eq!(artifact.uso_categoria(CATEGORIA_AUDIO), 100);
        assert_eq!(artifact.uso_categoria(CATEGORIA_COMPLEMENTOS), 40);
        assert_eq!(artifact.uso_categoria(CATEGORIA_PIPELINE), 100);
        let fracciones: Vec<u64> = artifact
            .categorias
            .iter()
            .map(|c| (c.fraccion * 1000.0) as u64)
            .collect();
        assert_eq!(fracciones, vec![333, 133, 333]);
        assert_eq!(artifact.transferencia.bytes_transferred, 1024);
    }

    #[test]
    fn comparacion_usa_union_de_intervalos_no_la_suma() {
        // Baseline serial: audio y complementos sin solape.
        let baseline = TimelineArtifact::from_timings(
            "baseline",
            "qobuz",
            &DownloadPhaseTimings {
                total_duration_ms: 300,
                bytes_transferred: 1,
                transfer_source: "network".to_string(),
                throughput_mibps: 1.0,
                phases: vec![
                    record(DownloadPhase::Transfer, 0, 200),
                    record(DownloadPhase::ResolveCover, 200, 250),
                    record(DownloadPhase::ResolveLyrics, 250, 300),
                ],
                ..Default::default()
            },
        );
        assert_eq!(baseline.union_ms, 300);
        assert_eq!(baseline.suma_etapas_ms, 300);
        // Candidata solapada: complementos durante la transferencia. La unión
        // se calcula con el mismo constructor, no con lógica duplicada.
        let candidata = TimelineArtifact::from_timings(
            "solapada",
            "qobuz",
            &DownloadPhaseTimings {
                total_duration_ms: 250,
                bytes_transferred: 1,
                transfer_source: "network".to_string(),
                throughput_mibps: 1.0,
                phases: vec![
                    record(DownloadPhase::Transfer, 0, 200),
                    record(DownloadPhase::ResolveCover, 25, 75),
                    record(DownloadPhase::ResolveLyrics, 75, 125),
                ],
                ..Default::default()
            },
        );
        assert_eq!(candidata.union_ms, 200); // [0,200) cubre todo
        assert_eq!(candidata.suma_etapas_ms, 300);
        assert_eq!(candidata.huecos_ms, 50);

        let cmp = baseline.compare_runs(&candidata);
        // Por suma ambos valen 300 y el delta sería 0: engañoso. Por unión,
        // la candidata cubre 200 ms de 300 del baseline: -100 ms reales.
        assert_eq!(cmp.total_union_baseline_ms, 300);
        assert_eq!(cmp.total_union_candidata_ms, 200);
        assert_eq!(cmp.delta_total_ms, -100);
        assert_eq!(cmp.solapamiento_candidata_ms, 100);
        let complementos = cmp
            .categorias
            .iter()
            .find(|c| c.categoria == CATEGORIA_COMPLEMENTOS)
            .expect("categoría complementos");
        // Los complementos de la candidata son contiguos entre sí: su unión
        // propia no cambia (100 ms, mismo trabajo); la mejora la captura el
        // total por unión, no la categoría aislada.
        assert_eq!(complementos.union_baseline_ms, 100);
        assert_eq!(complementos.union_candidata_ms, 100);
        assert_eq!(complementos.delta_ms, 0);
    }

    #[test]
    fn el_artefacto_no_guarda_credenciales_urls_ni_datos_privados() {
        let timings = DownloadPhaseTimings {
            total_duration_ms: 100,
            bytes_transferred: 5,
            transfer_source: "network".to_string(),
            throughput_mibps: 0.5,
            cache_hits: CacheHitReport {
                lyrics_hit: true,
                cover_hit: false,
                metadata_hit: false,
            },
            phases: vec![record(DownloadPhase::Transfer, 0, 100)],
            ..Default::default()
        };
        let artifact = TimelineArtifact::from_timings("baseline", "qobuz", &timings);
        let json = serde_json::to_string(&artifact).expect("serializa");
        let valor: serde_json::Value = serde_json::from_str(&json).expect("json válido");
        // Lista blanca de claves: cualquier clave nueva en el artefacto es una
        // decisión consciente de qué se mide, y nada ajeno se cuela.
        let claves: Vec<String> = valor.as_object().expect("objeto").keys().cloned().collect();
        let esperadas: Vec<String> = [
            "schema_version",
            "corrida",
            "servicio",
            "registrada_en",
            "total_pared_ms",
            "union_ms",
            "suma_etapas_ms",
            "huecos_ms",
            "transferencia",
            "categorias",
            "etapas",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let mut ordenadas = claves.clone();
        ordenadas.sort();
        let mut esperadas_ordenadas = esperadas.clone();
        esperadas_ordenadas.sort();
        assert_eq!(ordenadas, esperadas_ordenadas);
        // Marcadores de datos sensibles: ni URLs ni tokens ni campos privados
        // del resultado (título, artista, ISRC, rutas).
        for marcador in [
            "http",
            "Bearer",
            "token",
            "secret",
            "password",
            "credential",
            "file_path",
            "title",
            "artist",
            "isrc",
            "qobuz.com",
            "tidal.com",
        ] {
            assert!(
                !json.to_lowercase().contains(&marcador.to_lowercase()),
                "el artefacto no debe contener '{marcador}'"
            );
        }
    }

    #[test]
    fn reporte_explica_total_con_etapas_y_esperas() {
        let timings = DownloadPhaseTimings {
            total_duration_ms: 300,
            bytes_transferred: 10,
            transfer_source: "network".to_string(),
            throughput_mibps: 1.0,
            phases: vec![
                record(DownloadPhase::Transfer, 0, 100),
                record(DownloadPhase::ResolveCover, 100, 150),
                record(DownloadPhase::Persisting, 150, 250),
            ],
            ..Default::default()
        };
        let artifact = TimelineArtifact::from_timings("baseline", "qobuz", &timings);
        let informe = artifact.report();
        assert!(informe.contains("total pared: 300 ms"), "{informe}");
        assert!(informe.contains("unión de intervalos: 250 ms"), "{informe}");
        assert!(informe.contains("huecos sin atribuir: 50 ms"), "{informe}");
        assert!(informe.contains("audio"), "{informe}");
        assert!(informe.contains("solapables"), "{informe}");
    }

    #[test]
    fn comparacion_reporta_que_usa_union() {
        let baseline = TimelineArtifact::from_timings(
            "baseline",
            "qobuz",
            &DownloadPhaseTimings {
                total_duration_ms: 100,
                phases: vec![record(DownloadPhase::Transfer, 0, 100)],
                ..Default::default()
            },
        );
        let candidata = TimelineArtifact::from_timings(
            "solapada",
            "qobuz",
            &DownloadPhaseTimings {
                total_duration_ms: 100,
                phases: vec![
                    record(DownloadPhase::Transfer, 0, 60),
                    record(DownloadPhase::ResolveCover, 0, 40),
                ],
                ..Default::default()
            },
        );
        let cmp = baseline.compare_runs(&candidata);
        let informe = cmp.report();
        assert!(
            informe.contains("unión de intervalos, no sobre la suma"),
            "{informe}"
        );
        assert_eq!(cmp.total_union_candidata_ms, 60);
    }

    #[test]
    fn persist_artifact_hace_roundtrip() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let artifact = TimelineArtifact::from_timings(
            "baseline",
            "qobuz",
            &DownloadPhaseTimings {
                total_duration_ms: 50,
                bytes_transferred: 7,
                transfer_source: "network".to_string(),
                throughput_mibps: 0.7,
                phases: vec![record(DownloadPhase::Transfer, 0, 50)],
                ..Default::default()
            },
        );
        let ruta = persist_artifact(dir.path(), &artifact).expect("persiste");
        assert!(ruta
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("dlx-cronologia-baseline-qobuz-"));
        assert!(ruta.extension().unwrap() == "json");
        let leido: TimelineArtifact =
            serde_json::from_str(&std::fs::read_to_string(&ruta).expect("lee"))
                .expect("deserializa");
        assert_eq!(leido, artifact);
        // No deja temporales detrás.
        let restos: Vec<_> = std::fs::read_dir(dir.path())
            .expect("dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "tmp").unwrap_or(false))
            .collect();
        assert!(restos.is_empty());
    }

    #[test]
    fn sanitize_corrida_recorta_y_sustituye() {
        assert_eq!(sanitize_corrida("baseline-2026-11"), "baseline-2026-11");
        assert_eq!(sanitize_corrida(" corrida uno "), "corrida-uno");
        assert_eq!(sanitize_corrida("///"), "app");
        assert_eq!(sanitize_corrida(""), "app");
    }

    #[test]
    fn categoria_de_fase_cubre_todas_las_etapas() {
        for fase in [
            DownloadPhase::QueueWait,
            DownloadPhase::Auth,
            DownloadPhase::ResolveStream,
            DownloadPhase::Transfer,
            DownloadPhase::ValidateAudio,
            DownloadPhase::EnrichMetadata,
            DownloadPhase::ResolveLyrics,
            DownloadPhase::ResolveCover,
            DownloadPhase::Tagging,
            DownloadPhase::Promotion,
            DownloadPhase::Persisting,
        ] {
            assert!(
                categoria_de_fase(fase).is_some(),
                "{fase} debe pertenecer a una categoría de medición"
            );
        }
        assert!(categoria_de_fase(DownloadPhase::Completed).is_none());
        assert!(categoria_de_fase(DownloadPhase::Failed).is_none());
        assert!(categoria_de_fase(DownloadPhase::Cancelled).is_none());
    }
}
