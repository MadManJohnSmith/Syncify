# ADR-0018 — Letras: modelo de versiones por fuente+idioma con pin

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo LETRAS.
**Spec:** #176 · **Tickets:** #178–#194.

## Contexto

`lyrics` admite una fila por formato (`UNIQUE(track_id, format)`): last-write-wins
sin comparar calidad, sin concepto de versión activa. `get_lyrics` devuelve por
prioridad fija de formato `lrc > plain > ttml` (TTML pierde contra plain,
contradiciendo el comentario del código); el «historial de versiones» de
`LyricsView` son esas filas de formato, y «Restaurar» solo re-guarda la misma fila.
Las **Marcas de borrado** (LY-12b) se violan en cuatro rutas: batch, búsqueda de
faltantes, pipeline Tidal y re-fetch karaoke.

## Decisión

1. **Identidad de versión.** Una **Versión de letra** es (pista, **Fuente de
   letras**, idioma, formato); el formato es propiedad del contenido, no su
   identidad. Conviven las versiones de LRCLIB, Kugou, TTML, etc.

2. **Versión activa con pin.** El usuario puede fijar un **Pin de versión** por
   pista, persistido. Sin pin, la regla calcula la activa: **Nivel de sincronía**
   (más fino gana), luego idioma preferido, luego prioridad de fuente. La resolución
   automática y el re-fetch karaoke no degradan un pin. La regla elimina la
   anomalía `lrc > plain > ttml`.

3. **Solo datos nuevos, lectura en unión.** No se migran filas existentes: las
   escrituras nuevas van al almacén de versiones; las filas antiguas (una por
   formato) quedan intactas y se leen en unión para la regla, el pin y la UI. La
   tabla antigua deja de crecer; el dedup ISRC remapea ambos almacenes. Migración
   nueva; no se edita una aplicada.

4. **Marcas de borrado en todas las rutas.** Las cuatro rutas que hoy re-descargan
   lo borrado a mano dejan de hacerlo. La violación de LY-12b es un defecto, no una
   política: solo la acción explícita del usuario anula una marca.

5. **Embed solo FLAC en prebeta.** MP3 (ID3 USLT/SYLT) y M4A quedan postbeta,
   alineado con ADR-0009 (no hay escritor ID3; las descargas MP3 conservan la letra
   en sidecar/BD).

## Consecuencias

- `get_lyrics`, el editor, el re-fetch karaoke, el dedup y los pipelines consumen
  la unión de ambos almacenes (vista unificada de lectura).
- Coexisten para siempre dos almacenes de lectura; el antiguo es de solo escritura
  histórica.
- `lyrics_type` (hoy `CAST(NULL)`) pasa a dato real derivado de la versión activa.

## Alternativas descartadas

- **Rebuild con translado de filas.** Requería tocar datos existentes de usuarios.
- **Activa + tabla de archivo paralela.** Dos orígenes de verdad que sincronizar.
- **Regla sin pin.** El usuario no podría elegir qué versión suena.
- **Solo histórico append-only por formato.** Conserva la semántica
  last-write-wins que motiva el rediseño.
