# Handoff — Syncify: flujos de documentación

**Estado:** vivo · **Última revisión:** 2026-11-05

Fecha: 2026-11-05. Rama: `dev`.
Tracker: GitHub Issues de MadManJohnSmith/Syncify vía gh.

## Objetivo

Continuar el flujo por cada flujo de la app: /grill-with-docs → /to-spec → /to-tickets,
hasta cubrir prebeta y postbeta. Los nueve flujos de documentación están cerrados
(sync, cuentas, biblioteca, descargas, reproductor, playlists, migración, letras,
ajustes/backup); el plan post-beta quedó reconciliado. Queda implementación.

## Hecho (referencias, no duplicar)
- Documentación en GLOSSARY.md, docs/adr/, docs/agents/; puerta test_docs_consistency verde.
- ADR-0001..0003 migrados; ADR-0004 y ADR-0005 propuestos.
- Sync: ADR-0006, spec #64, tickets #65-#82.
- Cuentas: ADR-0007, spec #83, tickets #84-#95; login #62, sonda #63.
- Biblioteca: ADR-0008, spec #96, tickets #97-#110.
- Descargas: ADR-0004, ADR-0005 y ADR-0009 propuestos; spec #111; tickets #112-#128.
- Reproductor: ADR-0010 y ADR-0011 propuestos; spec #130; tickets #131-#149.
- Playlists: ADR-0012 y ADR-0013 propuestos; spec #152; tickets #153-#163.
- Migración: ADR-0015 y ADR-0016 propuestos; spec #164; tickets #165-#174.
- Letras: ADR-0017 y ADR-0018 propuestos; spec #176; tickets #178-#194.
- Ajustes/backup: ADR-0019 y ADR-0020 propuestos; spec #195; tickets #196-#202.

## Mapa de issues y dependencias
- Sync: T01=#65 ... T18=#82 (T08 cuarentena #72, T09 identidad #73, T15 reauth #79, T16 metadatos #80).
- Cuentas: A01=#84 ... A12=#95; A04 (reauth) depende de #79.
- Biblioteca: L00=#97 (pertenencia por colección/cuenta; depende de #65-#68),  L01=#98 (consulta v2; depende de #97 y #88), L02..#L13=#99-#110.
- Descargas: D00=#112, D01=#113, D02=#114, D03=#115, D04=#116, D05=#117, D06=#118, D07=#119, D08=#120, D09=#121, D10=#122, D11=#123, D12=#124, D13=#125, D14=#126, D15=#127, D16=#128.
- Arranque: D00 y D01 en paralelo; D02 espera a ambos; D14 cierra los puertos; D15 es la aceptación.
- Reproductor: P00=#131, P01=#132, P02=#133, P03=#134, P04=#135, P05=#136, P06=#137, P07=#138, P08=#139, P09=#140, P10=#141, P11=#142, P12=#143, P13=#144, P14=#145, P15=#146, P16=#147, P17=#148, P18=#149.
- Grafo del reproductor: P00 → {P01, P04, P07, P09, P16}; P01 → {P02, P03, P05}; P05 → P06; P07 → P08; P03 + #121 → P10; P10 → {P11…P15}; {P10, P16, #119} → P17; {P02, P04, P06, P08, P09, P15, P17} → P18.
- Playlists: PL00=#153, PL01=#154, PL02=#155, PL03=#156, PL04=#157, PL05=#158, PL06=#159, PL07=#160, PL08=#161, PL09=#162, PL10=#163.
- Grafo de playlists: PL00 → {PL01, PL03, PL06}; PL01 → {PL02, PL08}; PL02 → PL05; PL03 → PL04; {PL06, PL01, PL02, #74-#76} → PL07; {PL06, PL07} → PL09; {PL03, PL04, PL05, PL08, PL09} → PL10.
- Migración: M00=#165, M01=#166, M02=#167, M03=#168, M04=#169, M05=#170, M06=#171, M07=#172, M08=#173, M09=#174.
- Grafo de migración: M00 → M01; M01 → {M02, M03, M05, M08}; M03 → M04; M05 → {M06, M07}; {M02, M04, M06, M07, M08} → M09.
- Letras: LY00=#178 (spike matriz), LY01=#179 (crate), LY07=#180 (marcas), LY10=#181 (wipe), LY13=#182 (spike Tidal), LY02=#183 (selección), LY03=#184 (caché negativa), LY04=#185 (credenciales+sondas), LY06=#186 (cola cancelable), LY08=#187 (versiones+unión), LY05=#188 (retirada puente), LY09=#189 (pin+UI), LY11=#190 (auto-fetch), LY12=#191 (reintentos), LY14=#192 (Tidal nativo), LY15=#193 (copy+tipo), LY16=#194 (aceptación).
- Grafo de letras: frontera {LY00, LY01, LY07, LY10, LY13}; {LY00, LY01} → LY02; LY01 → {LY03, LY04, LY06, LY08}; LY04 → LY05; LY08 → {LY09, LY15}; {LY06, LY07} → LY11; {LY03, LY06, LY07} → LY12; {LY02, LY08, LY13} → LY14; {LY02, LY03, LY04, LY05, LY08, LY09, LY10, LY11, LY12, LY14, LY15} → LY16.
- Ajustes/backup: SB00=#196 (defaults+reset), SB02=#197 (ocultar huérfanos), SB03=#198 (cablear los que mienten), SB01=#199 (una superficie por dato), SB04=#200 (manifest v2 export), SB05=#201 (manifest v2 import), SB06=#202 (reconciliación+cierre).
- Grafo de ajustes/backup: frontera {SB00, SB02, SB03}; SB00 → {SB01, SB04}; SB04 → SB05; {SB00, SB01, SB02, SB03, SB04, SB05} → SB06.

## Pendiente
- DESCARGAS: documentación cerrada; implementación pendiente de #112-#128.
- REPRODUCTOR: documentación cerrada; implementación pendiente de #131-#149.
- PLAYLISTS: documentación cerrada; implementación pendiente de #153-#163.
- MIGRACIÓN: documentación cerrada; implementación pendiente de #165-#174.
- LETRAS: documentación cerrada; implementación pendiente de #178-#194.
- Arranque: D00 (#112), D01 (#113) y P00 (#131) en paralelo; PL00 (#153) también.
- AJUSTES/BACKUP: documentación cerrada; implementación pendiente de #196-#202.
- Documentación: los nueve flujos están cerrados; el plan post-beta quedó reconciliado
  (espera real: A-05, C-04, C-05, C-06, D-01/D-02, E-02 y los residuales parciales).

## Gotchas
- Descargas: ADR-0004 registra 17.969 favoritos, preflight + pasada real, modal no cancelable y 350 fallos/hora; los logs muestran 401/403 Tidal, no un stream unusable.
- ADR-0005: evidencia 30 días, denegación 1 hora, preferencia blanda y fuente/destino separados.
- ADR-0009: cinco destinos nativos; el seed dice supports_download=1 para Deezer/SoundCloud pero el orquestador los trata como catalog-only; SoundCloud entrega MP3 128; no hay escritor ID3; los puentes Python no se usan y la ruta SoundCloud no autentica.
- Reproductor: la cola vive en `usePlayer.ts` y el motor solo transporta una pista; ReplayGain está en `tracks` (0083) y nadie lo lee al sonar.
- Reproductor: S194 ya no excluye streaming (enmienda 2026-10-17) pero sí búsqueda; Apple Music y Spotify solo tienen `search_track`; favoritos tiene backend sin vista.
- Playlists: las locales cuelgan de la cuenta 1 con `local_<nombre>`; `UNIQUE(playlist_id, track_id)` prohíbe duplicados; `get_playlist_tracks` recorta a 500.
- Playlists: CASCADE borra las de una cuenta; solo Qobuz/Tidal crean y añaden; ningún proveedor tiene update/remove/reorder.
- Migración: recorre en serie con pausas de 25 ms y sin clave de idempotencia; un job `running` queda colgado al cerrar; una ruta de `MigrationView` fuerza `create_playlists: false`.
- Migración: solo Qobuz/Tidal crean playlists; álbumes y artistas varían por destino; no hay journal ni undo.
- Biblioteca: library_items es espejo de migración; library_stats cuenta filas; auto-fusión de duplicados contradice ADR-0008; playlists truncadas a 500; búsqueda en letras postbeta.
- Letras: motor v2 decidido en ADR-0017/0018; versiones solo para datos nuevos con lectura unión (filas antiguas intactas); las marcas de borrado se violan hoy en batch, faltantes, pipeline Tidal y re-fetch karaoke (corrige LY07).
- Letras: el batch no es cancelable (cerrar el diálogo no detiene el bucle); get_lyrics prioriza lrc > plain > ttml contra su propio comentario; residuos de Genius en el copy del player y del puente legacy.
- Ajustes: ~40 huérfanos visibles que nadie lee (duplicate_settings completo, advanced, metadata write_*, transcode, proxy, db_location); el reset solo limpia 6 superficies y la UI compensa en cliente; max_concurrent_downloads y la ruta de descarga se escriben en 2-4 sitios; el backup no incluye ajustes ni cuentas (desviación de ADR-0007 cláusula 11) hasta el manifest v2 (#200/#201).
- Sync: ningún proveedor tiene delta; orden no fiable; bugs de paginación; Apple fuera del motor; 834 Apple sin track_sources, 1.557 sin ISRC, 64 grupos de playlists duplicadas.

## Reglas del repo (AGENTS.md)
- No tocar ~/.local/share/com.syncify.app, biblioteca ni descargas reales.
- No editar migraciones aplicadas ni reutilizar números.
- No relajar puertas ni añadir excepciones.
- No regenerar .venv/node_modules/lockfiles sin motivo.
- Los logs de prueba manual pueden contener cuentas/biblioteca: no versionar ni compartir.

## Comandos
- python3 -m unittest scripts.tests.test_docs_consistency -q
- gh issue view 64 --json number,title,body,labels
- gh api repos/MadManJohnSmith/Syncify/issues/<n>/dependencies/blocked_by --jq "[.[].number] | sort"
- gh api repos/MadManJohnSmith/Syncify/issues/<n>/sub_issues --jq "length"

## Skills sugeridas
- grill-with-docs (grilling + domain-modeling) para el siguiente flujo (letras).
- to-spec y to-tickets para publicar y dividir.
- ask-matt si hay duda de skill; domain-modeling para glosario/ADR.

## Siguiente paso concreto
1. Implementar D00 (#112) y D01 (#113) en paralelo; después D02 (#114) y el resto del grafo. P00 (#131), PL00 (#153) y M00 (#165) pueden arrancar en paralelo.
2. Cerrar los spikes de D11 (#123) y D12 (#124) antes de escribir los adaptadores; sus incógnitas están en el ticket.
3. REPRODUCTOR: P00 (#131), después P01 (#132) y el grafo de arriba.
4. PLAYLISTS: PL00 (#153), después PL01 (#154) y el grafo de arriba.
5. MIGRACIÓN: M00 (#165), después M01 (#166) y el grafo de arriba.
6. LETRAS: LY00 (#178), LY01 (#179), LY07 (#180), LY10 (#181) y LY13 (#182) en paralelo; los spikes (#178, #182) cierran antes de LY02 (#183) y LY14 (#192).
7. AJUSTES/BACKUP: SB00 (#196), SB02 (#197) y SB03 (#198) en paralelo; después SB01 (#199), SB04 (#200) y SB05 (#201); SB06 (#202) cierra.
8. La documentación de los nueve flujos está completa: lo que queda es implementación de los cuatro grafos abiertos (descargas, reproductor, playlists, migración, letras, ajustes/backup) y el plan post-beta ya reconciliado espera al cierre del beta.
