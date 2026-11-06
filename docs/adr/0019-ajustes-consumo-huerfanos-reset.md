# ADR-0019 — Ajustes: consumo acreditado, huérfanos ocultos y reset único

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo AJUSTES/BACKUP.
**Spec:** #195 · **Tickets:** #196–#202.

## Contexto

Hay ~40 ajustes visibles que ningún código lee: los 7 de `duplicate_settings`
completos, 15 de `advanced_settings`, 5 y 11 de `metadata_preferences`, varios de
`audio_processing_settings`, `sync_include_appearances` y la KV `db_location`. El
reset del backend solo limpia 6 de las superficies y la UI compensa con defaults
duplicados en cliente (reconocido en un comentario del código). Los defaults viven
en tres sitios (seeds de migración, fallbacks Rust, copias UI) y hay valores
escritos en 2–4 superficies a la vez (`max_concurrent_downloads`, ruta de
descarga). El ledger `docs/auditoria/matriz-ajustes.md` ya advierte: "persistido"
acredita escritura, **no** consumo.

## Decisión

1. **Consumo acreditado.** Un ajuste existe solo con lector. La matriz mantiene su
   ratchet: ningún ajuste nuevo sin fila previa, y "persistido" nunca acredita
   consumo.

2. **Huérfanos: cablear u ocultar, nunca dejarlos.**
   - **Se cablean en este flujo** (baratos y hoy mienten): los flags de escritura
     de metadatos (`write_*`), `overwrite_on_reimport`, `preserve_custom_tags`,
     `multi_value_separator` y `sync_include_appearances`. `enable_lastfm` se
     cablea si existe el enriquecedor; si no, se oculta.
   - **Quedan visibles y los cablea su flujo:** `replay_gain_mode` y
     `target_loudness_lufs` (ya ticketeado en el reproductor, P07).
   - **Se ocultan** hasta que su feature exista: `duplicate_settings` completo,
     `transcode_*`/`keep_original_after_transcode`, `artwork_max_size`, la KV
     `db_location`, `enable_acoustid` y los huérfanos de `advanced_settings`
     (cache, fingerprinting, `use_proxy`/`proxy_url`, timeouts, logging). Cada uno
     queda registrado en la matriz con el plan que lo reabre.
   - `incremental_sync` se retira: decisión ya tomada por la spec de sync.

3. **Defaults en un solo sitio.** Un módulo único de defaults por superficie
   alimenta seeds, lecturas de fallback y reset; las copias de defaults en la UI
   se eliminan.

4. **Cada dato en una sola superficie.** Se mantiene la dualidad KV/tipadas, pero
   `max_concurrent_downloads` y la ruta de descarga dejan de escribirse en varias;
   las lecturas toleran las claves legacy ya escritas. La consolidación total a
   tablas tipadas no entra en este flujo.

5. **Reset completo y único en backend.** Un comando backend resetea todas las
   superficies tipadas y la KV de ajustes; la UI deja de compensar en cliente.

6. **Higiene.** Se elimina el comando con atributo pero sin registro y se alinea
   la pareja de calidad global con su superficie real.

## Consecuencias

- Ocultar no borra columnas: no se edita ninguna migración aplicada; los ajustes
  ocultos vuelven cuando su feature lo pida (transcode, proxy, duplicados…).
- El reset pasa a ser verificable como contrato de backend, no como suma de
  compensaciones de UI.
- Los tests de copy veraz existentes se extienden a las superficies afectadas.

## Alternativas descartadas

- **Cablear todo.** La mayoría de huérfanos depende de features inexistentes
  (cache, fingerprinting, proxy, transcode): sería cablear contra código que no
  está.
- **Consolidar toda la KV a tablas tipadas.** Expand-contract de gran radio;
  excede el alcance de un flujo de honestidad de ajustes.
- **Dejarlos visibles y documentarlos.** Contradice la política de copy veraz que
  el repo ya aplica y testea.
