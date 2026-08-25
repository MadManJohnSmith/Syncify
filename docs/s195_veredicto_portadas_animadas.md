# S195(d) — Veredicto: portadas animadas y soporte WebP

**Pregunta del propietario:** «¿Es por el formato de la pista? ¿Solo se puede incrustar WebP si es FLAC?»

## Respuesta corta

Sí, en la práctica: **FLAC** admite incrustar WebP (estático e incluso animado) como bloque `METADATA_BLOCK_PICTURE`; **M4A/AAC no** (el átomo `covr` de estilo iTunes solo define JPEG/PNG). Y las portadas *animadas* que se ven en Tidal/Qobuz/Apple Music **no son imágenes de la pista: son videos editoriales del proveedor** que ningún contenedor de audio permite incrustar como portada.

## Evidencia de código (solo lectura)

### FLAC — ya soporta WebP hoy
- `crates/syncify-core-domain/src/cover_rules.rs:25-32` — `CoverType::mime_type()` mapea `AnimatedWebp | StaticWebp → "image/webp"`, además de jpeg/png.
- `crates/syncify-core-domain/src/byte_validators.rs:77-100` — `detect_cover_type()` detecta por magic bytes RIFF/WEBP, incluida animación (VP8X/ANMF).
- `crates/syncify-flac-writer/src/lib.rs:505-523` — el writer YA incrusta WebP con mime correcto y aplica el invariante: un CoverFront WebP animado existente **nunca** se sobrescribe con JPEG/PNG estático entrante.
- Conclusión: incrustar WebP estático en FLAC **no solo es viable y barato: ya está implementado**. Si una portada llega como WebP a la ruta FLAC, se incrusta tal cual.

### M4A — covr limitado a JPEG/PNG
- `src-tauri/src/services/mp4_writer.rs:394-403` — el átomo `covr` decide: `\x89PNG` → `Data::Png`, **todo lo demás → `Data::Jpeg`**. No existe variante WebP en el formato covr iTunes (tipos 13/14). Un WebP entrante se escribiría mal clasificado como JPEG (archivo inválido para muchos players).
- Decisión correcta: **en M4A mantener JPEG/PNG**, convertir si hace falta.

### Portadas animadas = assets de video del proveedor
- Tidal: la única portada que el pipeline descarga/incrusta viene de `https://resources.tidal.com/images/{uuid}/320x320.jpg` (`src-tauri/src/services/tidal.rs:78-83`) — **JPEG estático siempre**. Las portadas animadas de Tidal existen aparte, como clips de video editoriales servidos por endpoints de video (.mp4/HLS), nunca como imagen incrustable.
- Apple Music: `src-tauri/src/services/animated_cover.rs:1-14` — documenta el único flujo animado implementado: extrae token del web player, consulta `editorialVideo.motionDetailSquare.video` (HLS .m3u8), **convierte el video a WebP animado con ffmpeg** y lo incrusta en FLAC + sidecars (`cover.webp`, `folder.webp`, `animated.webp`).
- Qobuz: no ofrece portadas animadas; solo JPG estáticos.

## Veredicto técnico

Ningún contenedor de audio permite incrustar un VIDEO como portada:
- El bloque PICTURE de FLAC exige `image/*`; un H.264/HEVC no es imagen válida.
- `covr` en MP4/M4A solo define JPEG(13)/PNG(14).
- La vía válida —ya implementada para Apple Music— es transcodificar el video editorial a WebP animado (ffmpeg) e incrustarlo en FLAC o dejarlo como sidecar.

## Mensaje sugerido para el propietario

> Las portadas animadas que ves en la app de Tidal/Qobuz/Apple Music son videos promocionales del proveedor, no imágenes de la pista; ningún formato de audio (FLAC ni M4A) permite incrustar un video como carátula. Lo que sí hace Syncify: en FLAC acepta WebP (estático y animado) como carátula — ya está implementado y protegido contra sobreescritura; en M4A el estándar solo permite JPEG/PNG. De los proveedores conectados, solo Apple Music entrega ese video animado (se convierte a WebP animado); Tidal siempre entrega JPG estático, así que lo que ves animado en su app no puede viajar dentro del archivo.
