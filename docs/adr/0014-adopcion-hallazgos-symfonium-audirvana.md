# ADR-0014 — Adopción selectiva de hallazgos de Symfonium y Audirvāna

**Estado:** aceptado · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo REPRODUCTOR, a partir de los informes de
ingeniería inversa de Symfonium 15.1.0 y Audirvāna Studio/Origin/Remote.
**Spec:** #130 · **Tickets:** #131–#149; la enmienda añade trabajo y los tickets se
ajustan al implementar.

## Contexto

Syncify está cerrando el flujo REPRODUCTOR con dos ADR propuestos que esta ronda
acepta: ADR-0010 (cola en backend y búsqueda en proveedores) y ADR-0011 (motor
DSP). En paralelo se
hicieron dos trabajos de ingeniería inversa estática:

- **Symfonium 15.1.0**: contrato de tags y metadatos, modelo de proveedores,
  reproducción y casting. Lo relevante para Syncify es el contrato de tags, porque
  el escritor FLAC ya se declara compatible con Symfonium.
- **Audirvāna Studio 3.4.0 / Origin 3.1.0 / Remote 6.2.2**: pipeline DSP, cola
  persistida, streaming de servicios, búsqueda multi-fuente, API remota y salidas
  UPnP/Chromecast. Lo relevante es el reproductor y el streaming.

No todo el trabajo aplica. Symfonium es una app Android con USB DAC/DSD,
telemetría y capas de sistema; Audirvāna es un reproductor audiophile con MQA,
DSD, ASIO/WASAPI, multizona y licencias. Este ADR fija qué se adopta, qué se
difiere y qué se descarta.

## Decisión

### 1. Criterio

Se adopta solo lo que (a) corrige un defecto confirmado o (b) completa una
decisión que ya está en ADR-0010/0011. Lo que añade una capacidad nueva se
difiere a post-beta o se descarta. No se implementa nada en esta ronda: este ADR
es el registro de la decisión.

### 2. Contrato de tags (adoptar ahora, corrección)

El escritor FLAC/MP4 ya existe; estos cambios corrigen el contrato, no añaden una
feature nueva. Por eso este ADR no enmienda ADR-0002.

| Campo | Decisión | Condición |
|---|---|---|
| `REPLAYGAIN_TRACK_GAIN`, `REPLAYGAIN_TRACK_PEAK` | Sin cambio de formato | — |
| `REPLAYGAIN_ALBUM_GAIN`, `REPLAYGAIN_ALBUM_PEAK` | Ganancia de álbum real | Solo si se analizó el álbum completo; si no, `NULL` |
| `R128_TRACK_GAIN`, `R128_ALBUM_GAIN` | Entero Q7.8 (`round(dB * 256)`), nunca `"-X.XX LU"` | Siempre que se escriba R128 |
| `ITUNESADVISORY` | Escribir para contenido explícito; conservar `EXPLICIT` | Si `explicit == true`; MP4 `rtng` |
| `DATE` | Escribir junto a `YEAR` y `RELEASEDATE` | Si hay fecha o año |
| `ALBUMSORT`, `ARTISTSORT`, `ALBUMARTISTSORT`, `COMPOSERSORT` | Escribir | Solo si la fuente trae el dato |
| `MOVEMENTNAME`, `MOVEMENTNUMBER`, `MOVEMENTCOUNT` | Escribir | Solo si la fuente trae el dato |
| `CONDUCTOR`, `LYRICIST`, `PRODUCER`, `ENGINEER`, `MIXER`, `ARRANGER` | Escribir | Solo si la fuente trae el dato |
| `RATING` / `POPM` | Escribir | Solo si la fuente trae el dato |
| `RELEASETYPE` | Normalizar a los valores enumerados en minúscula | Verificar antes si el lector distingue mayúsculas |
| `ARTIST`, `GENRE`, `STYLE`, `MOOD`, `TAGS` | Mantener valores repetidos; no unir con separadores | Sin cambio |

Regla transversal: no se inventa ningún valor. Si la fuente no lo trae, el campo
no se escribe. La ganancia de álbum fabricada (`track + 0.70 dB`) se elimina.

Los campos `ORIGINALALBUM`, `ORIGINALARTIST`, `OCCASION` y `ALBUMVERSION` quedan
**diferidos**: se escribirán solo si la fuente los trae y si el flujo de metadatos
los expone; no son parte de la corrección inmediata.

### 3. Reproductor y cola (adoptar ahora; enmienda ADR-0010/0011)

- **Eventos**: instantánea completa al montar la webview, reconectar MPRIS o abrir
  la bandeja; deltas para cambios de estado. `PositionTick` no se persiste.
- **Shuffle**: la cola mantiene su orden estable; shuffle genera un orden de
  reproducción aparte y persistido. Al apagar shuffle se vuelve al orden de cola.
- **Preflight**: antes de encolar o reproducir una entrada efímera se consulta
  entitlement y estado de la cuenta; si no puede sonar, se marca "reconectar" en
  vez de fallar a mitad.
- **Prefetch y caducidad**: la siguiente entrada se resuelve antes de que termine
  la actual; un 401/403/410 provoca **una** re-resolución/refresh y, si falla, un
  error tipado.
- **Búsqueda de proveedores**: resultados agrupados por servicio, con estado
  connected/expired/preview-only/unavailable y acciones reproducir/encolar/descargar
  según capacidad. No se copia la API remota de Audirvāna.
- **Streaming**: `player:stream-state` lleva `errorKind`, `serviceName`,
  `accountId` y `statusCode`, además de la calidad de fuente y de salida.
- **DSP**: EQ con tipos de banda (peak, low/high shelf, low/high pass), indicador
  de clipping y orden fijo EQ → ganancia → limiter; sin crossfeed, convolver ni
  VST3. El modo álbum usa ganancia real o cae a track.
- **Calidad de salida**: el motor reporta sample rate, bit depth, canales y si
  hubo resample; no se presenta la calidad de fuente como si fuera la de salida.

El detalle vive en las enmiendas de ADR-0010 y ADR-0011.

### 4. Diferido a post-beta

| Capacidad | Decisión | Disparador / nota |
|---|---|---|
| FTS5 para búsqueda local | Diferido, condicionado a medición | Si `LIKE` degrada a escala (referencia 30k pistas); mismo contrato de comandos |
| Tope de calidad de reproducción | Diferido | Global + override por cuenta; mantiene la etiqueta honesta |
| Casting UPnP + Chromecast | Diferido; ADR propio | Ambos a la vez; proxy local con token y range; local + Qobuz/Tidal; DASH de Tidal a FLAC progresivo; sin promesa de gapless |
| Servidores personales Plex/Jellyfin | Diferido; ADR propio | Solo lectura (navegar/buscar/reproducir); entradas efímeras + promoción; sin descargas ni escrituras; Emby/Subsonic/Kodi después |
| Abstracción de proveedor | Diferido con los dos anteriores | Se extrae cuando se añada el tercer proveedor de reproducción |
| `ORIGINALALBUM`, `ORIGINALARTIST`, `OCCASION`, `ALBUMVERSION` | Diferido, condicional | Solo si la fuente los trae |

Ninguna de estas capacidades entra en el alcance beta; este ADR no enmienda
ADR-0002 (la enmienda de búsqueda ya vive en ADR-0010).

### 5. Descartado

- MQA, DSD/SDM/DoP y reproducción exclusiva USB/ASIO/WASAPI **como feature de esta
  ronda**. Bit-perfect sigue abierto (ver abajo).
- Multizona, VST3, convolver, crossfeed, balance y polaridad.
- Radio/podcasts Airable, SMB/WebDAV y almacenamiento en nube.
- La API remota de Audirvāna (pairing, REST, WebSocket) y sus anti-patrones de
  seguridad: HTTP/WS en claro, tokens sin expiración y secretos en el almacén de
  la app sin keyring del SO. Syncify mantiene AES-GCM + keyring.
- Las capas Android de Symfonium: firma, telemetría, `ApiReceiver`, USB DAC/DSD,
  el servidor HTTP de casting tal cual (sin auth y con bind comodín) y Compose/TV.
- Reintentar 401/403 como si fueran transitorios: es el defecto que se corrige.

**No descartado:** la reproducción bit-perfect/exclusiva sigue siendo una pregunta
abierta de producto; si se aborda, necesita su propio ADR.

### 6. Preguntas abiertas

- ¿La reproducción debe ser bit-perfect o se acepta resample? Hoy rodio usa el
  default del dispositivo y no se reporta.
- ¿`RELEASETYPE` distingue mayúsculas en Symfonium? Si no, la normalización es
  innecesaria; si sí, es un defecto de compatibilidad.
- ¿Qué hacer cuando no se analizó el álbum completo? La decisión es `NULL` +
  fallback a track, pero falta definir el aviso de UI.
- ¿El casting incluirá streams de servidores personales? Hoy el alcance es local +
  Qobuz/Tidal; Plex/Jellyfin queda para cuando exista ese workstream.

## Consecuencias

- ADR-0010 y ADR-0011 pasan a **aceptado** y llevan una enmienda con los puntos
  adoptados.
- `GLOSSARY.md` gana "calidad de fuente", "calidad de salida", "preflight de cola",
  "estado de proveedor", "servidor personal" y "renderer".
- No se toca código en esta ronda; la implementación sigue en #131–#149 y en los
  tickets que se abran para tags.
- Los informes de análisis siguen siendo la evidencia; este ADR es la decisión.
- Casting, servidores personales, FTS5 y tope de calidad necesitan su propio ADR
  antes de implementarse.

## Alternativas descartadas

- **Solo correcciones de datos (paquete A).** Deja el modo álbum del ADR-0011 con
  un valor inventado y el streaming con reintentos 401/403 contradictorios.
- **Adoptar todo ahora.** Mete casting, servidores personales y FTS5 en beta, en
  tensión con ADR-0002.
- **Dejar el análisis como referencia sin decisiones.** No corrige R128 ni la
  ganancia de álbum, que ya afectan a archivos descargados.

## Evidencia

- Informe de Symfonium: *SYNCIFY-FINDINGS.md* en la carpeta de análisis de
  Symfonium; guía de tags *TAGGING-GUIDE.md* y *REPORT.md*.
- Informe de Audirvāna: *SYNCIFY-FINDINGS.md* en la carpeta de análisis de
  Audirvāna; *Audirvana_Studio_3.4.0_Reverse_Engineering_Report.md*,
  *Audirvana_Remote_6.2.2_APK_Reverse_Engineering_Report.md* y
  *native-live/FINDINGS.md*.
