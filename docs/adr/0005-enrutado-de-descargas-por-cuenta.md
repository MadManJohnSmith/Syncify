# ADR-0005 — Enrutado de descargas por derecho de cuenta y evidencia reutilizada

**Estado:** propuesto · **Fecha:** 2026-11-04
**Origen:** requisito del propietario sobre el sistema multicuenta y multiservicio.
Responde a una pregunta abierta del ADR-0004.

## Contexto

La biblioteca centraliza la identidad de la pista, pero **descargar es una operación
por cuenta**: una pista puede estar identificada en el catálogo y aun así no ser
servible por la cuenta que el worker elija. Hay 14 cuentas repartidas en 7 servicios.

Los servicios funcionan por suscripción. El derecho a servir una pista depende de la
suscripción, de la región y del catálogo, y cambia con el tiempo; no es una propiedad
estable de la pista. La compra puntual de un álbum es la excepción, no la regla.

La evidencia actual apunta a que el worker elige la cuenta activa del servicio: la
cuenta Tidal activa no tiene ninguna entrada de biblioteca y la cola acumuló 350 fallos
en una hora, 308 de ellos «Tidal pipeline failed: Stream resolution rejected or
unavailable». No está probado que esa sea la única causa, pero el patrón encaja.

Los logs de esa prueba manual lo confirman: los fallos Tidal son `PlaybackUnauthorized`
(HTTP 401/403) en todos los endpoints de playback de todas las cuentas candidatas, y
Qobuz registra `QobuzPreviewPayload` por entregar MP3 (format_id=6) cuando se pidió
lossless. No es un problema de resolución de stream: es falta de derecho de las cuentas
usadas.

Sondear cada cuenta de cada servicio por cada pista es inviable a esta escala: 17.969
pistas favoritas distintas por 14 cuentas.

## Decisión

- Cada descarga se enruta a **una** combinación servicio + cuenta, elegida por
  evidencia, nunca por sondeo exhaustivo.
- Una entrega correcta es **evidencia reutilizable**: si una cuenta sirvió esa pista,
  se prefiere esa cuenta para esa pista y ese servicio.
- La vigencia del derecho se trata como **estado con caducidad**. La evidencia vieja no
  se usa a ciegas: si la entrega falla, se revalida.
- La **preferencia de calidad explícita** manda sobre el resto del orden. Si se pidió
  Qobuz a 24 bits, se intenta primero Qobuz y en esa calidad.
- Un 401/403, una caída de calidad no permitida o un rechazo de stream invalidan la
  evidencia de **esa cuenta para esa pista**, no la de todas las cuentas ni la de todas
  las pistas.

## Decisiones del propietario (ronda 1, 2026-11-05)

- **Autoridad de enrutado:** híbrida. La preferencia explícita del usuario manda
  (cuenta, servicio y calidad, a nivel de job); sin preferencia, decide la evidencia.
  No se elige cuenta por pista ni se sondean todas las cuentas.
- **Fallos clasificados:** un transitorio (red, timeout, 5xx) se reintenta con backoff
  y límite. Uno permanente (401/403, stream rechazado, no disponible en esa cuenta) no
  se reintenta contra la misma cuenta: se enruta a otra y, si no hay, se marca el
  fallo. El reintento manual sigue disponible.
- **Favoritos:** «descargar favoritos» es un job de primer nivel en beta; el enrutado
  debe sostener decenas de miles de pistas sin sondeo exhaustivo (ADR-0004).
- **Caducidad de evidencia:** 30 días por defecto, configurable por proveedor; un
  fallo la invalida al instante. La evidencia caducada no se usa a ciegas.

- **Preferencia de cuenta:** blanda. La cuenta preferida se intenta primero; si falla
  de forma permanente, se enruta a otra y se declara en el item. Ajustes define un
  default por servicio y el job puede sobrescribirlo.
- **Calidad:** mandan los ajustes (techo global, preferencia por servicio y
  `fallback_action`); el job puede pedir una calidad concreta que prevalece. El worker
  debe aplicar `fallback_action`, no ignorarlo.
- **Denegaciones:** un rechazo (401/403, calidad no permitida) invalida la evidencia de
  esa cuenta y pista, y se cachea como denegación durante una hora.
- **Fuente y destino:** el filtro de fuente (servicio de la biblioteca) y la preferencia
  de destino (proveedor que entrega) son selectores distintos. El destino se elige por
  preferencia explícita y, si no, por evidencia.
- **Proveedores:** todos los servicios con descarga declarada tendrán adaptador nativo
  en Rust (Qobuz, Tidal, Deezer, SoundCloud y Amazon); Spotify y Apple Music quedan como
  catálogo. Ver ADR-0009.

## Modelo de datos propuesto

Hoy `track_sources` es por servicio, no por cuenta, y describe catálogo, no derecho.
Propuesta aditiva:

- Una tabla de derechos por cuenta y pista, con servicio, calidad, estado, origen de la
  evidencia, instante de comprobación y caducidad.
- Estados: `granted`, `denied`, `unknown`, `stale`.
- El origen distingue `delivered` (la cuenta entregó la pista), `probe` (se comprobó
  explícitamente) y `sync` (lo dejó la importación).
- Índices por pista y servicio, y por cuenta y caducidad.
- No se toca `track_sources` ni la identidad canónica.

## Política de selección propuesta

1. Filtrar servicios que ofrecen la calidad pedida.
2. Si hay evidencia vigente `granted` para esa pista, usarla.
3. Si no, elegir la cuenta con credenciales válidas, empezando por la que más entregas
   recientes tenga en ese servicio; nunca recorrer todas las cuentas de todos los
   servicios.
4. Un intento por pista y servicio. Si falla, pasar al siguiente servicio permitido por
   la preferencia.
5. Cachear también el fallo durante una hora para no repetir sondeos.

## Consecuencias

- El número de llamadas baja de «pistas por cuentas» a «pistas» más revalidaciones.
- La cola deja de fallar por elegir una cuenta sin derecho.
- Hace falta una migración aditiva y una ventana de revalidación configurable por
  proveedor.
- Persistir derechos por cuenta añade filas; hay que medirlo con la biblioteca real.

## Preguntas abiertas

- ¿Qué cuenta como evidencia: descarga completada, URL de stream obtenida o sonda
  explícita?
- ¿La ventana de 24 horas es real en cada proveedor o una convención? Debe ser
  configurable y medible, no un número universal.
- ¿Se rota entre cuentas con derecho para repartir uso, o se fija la que funcionó?
- Si no hay derecho a la calidad pedida, ¿se degrada a lossy o se falla?
- ¿La evidencia de entrega sobrevive a una reautorización de la cuenta?
