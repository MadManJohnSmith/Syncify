# ADR-0006 — Contrato de sincronización: modo configurable, disparo y cuentas

**Estado:** propuesto · **Fecha:** 2026-11-04
**Origen:** ronda de grilling sobre sincronización, con la biblioteca real (31.359
pistas, 14 cuentas, 6 servicios).
**Spec:** [#64](https://github.com/MadManJohnSmith/Syncify/issues/64) · **Tickets:** [#65](https://github.com/MadManJohnSmith/Syncify/issues/65)–[#82](https://github.com/MadManJohnSmith/Syncify/issues/82).

## Contexto

La sincronización actual es unidireccional y acumulativa: cada brazo recorre los
endpoints del proveedor y hace upsert, pero ninguna línea borra una fila porque el
proveedor ya no la devuelva (A-01). Un favorito desmarcado sigue favorito para
siempre; una pista retirada de una playlist se queda en `playlist_tracks`.
`incremental_sync` se persiste y se muestra en Ajustes, pero ningún código lo lee
(A-03). El scheduler existe pero no tiene interfaz (`auto_sync_enabled=false`), y el
único «sync de todo» de la bandeja solo toca la cuenta activa de cada servicio. El
glosario define Sync como «importar favoritos o likes», aunque los brazos también
importan playlists, álbumes y artistas.

## Decisión

1. **El modo de sincronización es configurable, con «solo importar» por defecto.** El
   modelo admite cuatro contratos: solo importar (upsert sin detectar bajas), espejo
   con cuarentena, espejo estricto y bidireccional. El valor por defecto es solo
   importar: la beta no cambia de comportamiento al actualizar. Cada modo está
   limitado por las capacidades declaradas del proveedor (A-08): ninguno promete lo
   que su API no expone.
2. **Ningún modo borra catálogo.** El espejo se materializa como cuarentena: lo que ya
   no está en el proveedor se marca, no se elimina. `tracks`, `track_sources`,
   `library_entries` y el audio descargado sobreviven (coherente con A-01 y el
   ADR-0003).
3. **Disparo manual + intervalo configurable.** El botón manual siempre está. El
   auto-sync es opt-in, con intervalo editable y pausa; conectar una cuenta dispara la
   sincronización de esa cuenta. Se expone el scheduler existente en Ajustes en vez de
   crear otro.
4. **La unidad de configuración es la cuenta, con el servicio como default.** Cada
   servicio fija el modo, el alcance y la cadencia; cada cuenta hereda y puede
   sobrescribir. El toggle de participación de una cuenta gobierna tanto las corridas
   automáticas como «sincronizar todo»; un «sync ahora» por cuenta siempre puede
   forzar una corrida.
5. **«Sincronizada» tiene dos ejes, no uno.** El estado de la corrida
   (terminó / parcial / falló) describe la ejecución; la cobertura (reportado vs
   importado, o desconocida si el proveedor no publica total) describe cuánto falta.
   Una corrida sin errores con huecos es parcial, no verde.
6. **La cuarentena conserva, marca y oculta.** Una pista que el proveedor ya no
   devuelve mantiene su fila, sus fuentes y su audio; se excluye de listados y
   contadores por defecto, aparece bajo el filtro «ya no está» y las playlists
   muestran el hueco marcado. Lo mismo aplica a pertenencias y a playlists ausentes.

7. **El alcance es todo lo que el proveedor expone, activable por servicio.** Favoritos y
   playlists están siempre activos; álbumes, artistas, historial y compras se activan
   por servicio según las capacidades declaradas (A-08). El default importa el máximo
   disponible de cada proveedor; un proveedor sin una colección no la simula.
8. **Los metadatos se rellenan, no se pisan.** Un valor no nulo del catálogo local no se
   sobrescribe automáticamente; las contradicciones materiales del proveedor (artista,
   álbum, año) van a `needs_review` con la evidencia de ambos lados.
9. **Progreso por cuenta y fase, con historial.** La interfaz muestra cuenta y fase en
   curso, vistos/importados, tiempo y cancelación. Al terminar queda la última corrida
   por cuenta con estado, cobertura y errores; se conservan las N últimas.
10. **El auto-sync recupera y reintenta sin bloquear.** Si el intervalo venció con la
    app cerrada, la corrida ocurre al abrir; los fallos reintentan con backoff
    exponencial y no bloquean otras cuentas; sin red se pausa y reanuda al reconectar.

11. **Recorrido completo con huellas, sin parada temprana.** Cada corrida recorre todas
    las páginas de cada colección. Las huellas disponibles (Spotify `snapshot_id`,
    Deezer `mod_date`, SoundCloud `last_modified`, conteo en Qobuz y Tidal) evitan
    expandir playlists sin cambios (A-02). No se corta al encontrar ítems conocidos:
    el orden de los proveedores no permite saber dónde termina lo nuevo.
12. **Las bajas se detectan por conciliación de conjuntos.** El barrido recoge los ids
    que el proveedor devuelve y, al final, se comparan con la pertenencia local; lo
    ausente va a cuarentena. No cuesta llamadas extra porque el barrido ya ocurre.
13. **El modo espejo concilia en cada corrida del modo**, sea manual o automática. No
    hay una cadencia de reconciliación separada: el modo gobierna.
14. **Ritmo conservador y serial por servicio.** Se respetan los límites documentados
    con margen; las cuentas del mismo servicio se serializan porque comparten cuota de
    aplicación, y servicios distintos pueden correr en paralelo.
15. **Identidad de playlists: id dentro del servicio, huella entre servicios.** Renombrar
    una playlist no crea una playlist nueva: el id del proveedor es la identidad dentro
    del servicio. La huella nombre + pistas fusiona la misma playlist entre servicios;
    si las pistas divergen, se separan o van a revisión.
16. **Identidad de pistas: ISRC primero, huella después.** Un ISRC idéntico enlaza
    automáticamente. Sin ISRC se compara artista + título + duración; lo ambiguo va a
    `needs_review`. Nunca se fusiona por título solo.
17. **Los fallos no abortan la corrida.** Cada página se reintenta con backoff; si
    persiste el fallo, se registra el hueco, se continúa con el resto y la corrida queda
    «parcial». Lo ya importado se conserva.
18. **El modo bidireccional queda modelado, deshabilitado y planificado para post-beta.**
    La interfaz lo muestra como no disponible con el motivo por proveedor. Implementarlo
    exige ampliar scopes (Spotify no pide `user-library-modify` en el flujo actual) y un
    flujo de escritura que no existe.

19. **Apple Music se unifica bajo el mismo contrato.** Entra por el orquestador común
    con progreso, preferencias, cuenta elegida, cuarentena y estados; se adapta su
    importador en vez de mantener la excepción.
20. **Una credencial inválida detiene solo esa cuenta.** Ante 401/403, la cuenta queda
    fallida con motivo, se conserva lo importado y se marca para reautenticar; las
    demás cuentas continúan.
21. **Solo se notifican fallos y parciales.** Un sync correcto no interrumpe; una cuenta
    fallida, parcial o pendiente de reautenticación avisa por bandeja o en la app.
22. **Se conservan las últimas 20 corridas por cuenta**, visibles en su ficha con
    estado, cobertura, duración y errores; se poda lo anterior.

23. **El conflicto de playlists es configurable por playlist.** Cada playlist remota puede
    estar en uno de tres modos —editable, gana el proveedor o gana local— y la ficha de la
    playlist permite cambiarlo. El servicio fija el modo inicial de las playlists nuevas.
24. **Retirar un favorito no retira la pista.** Si la pista sigue en una playlist, la
    entrada de favorito va a cuarentena y el contador de favoritos baja; la pista y su
    pertenencia a la playlist se conservan.
25. **El sync manual tiene tres granularidades:** cuenta, colección (favoritos, playlists,
    álbumes, artistas) y playlist individual.
26. **Toda aparición es evidencia.** Se registra la cuenta y el servicio de cada aparición
    y alimenta el enrutado de descargas del ADR-0005.

27. **Por defecto gana el proveedor, con aviso al editar.** Una playlist remota refleja su
    origen; la primera edición local pregunta si se cambia a editable o a local, para no
    perder cambios en silencio.
28. **«Editable» significa editar y revisar conflictos.** Se permite renombrar, reordenar y
    quitar pistas; si el proveedor cambia lo mismo, el conflicto va a `needs_review` y
    nada se pisa en silencio.
29. **Una aparición se pone en cuarentena con su origen.** Si la playlist desaparece o la
    cuenta se desvincula, la evidencia se conserva con fecha y deja de contar para el
    enrutado activo; si el origen reaparece, se restaura.

30. **El auto-sync propone 24 horas por defecto.** El intervalo es configurable y se mide
    en horas; 24 h mantiene fresco el catálogo sin castigar cuotas, y el usuario puede
    bajarlo.
31. **Las corridas concurrentes se encolan por servicio.** Una petición que comparte
    servicio con la corrida activa espera su turno; una de otro servicio arranca en
    paralelo con su propio cupo. Nunca se cancela una corrida en curso para empezar otra.
32. **Se enriquece toda pista distinta, con huellas.** Favoritos, playlists, álbumes y
    artistas; cada pista se enriquece una vez y las playlists sin cambios se saltan con
    A-02. No se limita a lo que entra a biblioteca.
33. **Las apariciones se conservan sin poda.** La cuarentena invalida la evidencia cuando
    desaparece el origen, pero no se borra por antigüedad; el volumen medido (192.477
    pertenencias) no lo exige.

34. **Las copias de playlist se reconocen por contenido, no por procedencia.** Una
    playlist canónica puede tener varias fuentes, incluso dos de la misma cuenta y
    servicio. Se fusionan automáticamente las que comparten nombre normalizado y conjunto
    exacto de pistas; las que comparten nombre y ≥90 % de pistas se marcan como «posible
    duplicado» para revisión, sin fusionar; las de nombre igual pero contenido distinto
    siguen separadas. La fusión cruza cuentas del mismo servicio cuando el contenido es
    idéntico; los contadores siguen siendo por cuenta. La procedencia de una migración no
    es requisito para reconocer la copia: no se guarda el id de la playlist de origen y la
    evidencia es el contenido.


### Reglas derivadas

- Cancelar una corrida la detiene en el límite de página, conserva lo importado y la
  marca «cancelada». No hay reanudación a mitad: la siguiente corrida vuelve a barrer.
- Si una entidad en cuarentena reaparece en el proveedor, la marca se retira en la
  siguiente conciliación.
- La cobertura se calcula por colección (favoritos, playlists, álbumes, artistas) y se
  agrega por cuenta; un total ausente se muestra como desconocido, no como 100 %.

## Evidencia de los proveedores (verificada el 2026-11-04)

- **Ninguno expone delta.** Los seis exigen recorrer con `offset` o cursor; no hay
  `since`, `updated_after` ni sync token.
- **Totales:** los seis pueden reportar el total de favoritos, aunque SoundCloud lo
  publica en el objeto de usuario (`likes_count`), no en la colección.
- **Orden:** Apple devuelve alfabético; Deezer devuelve las favoritas de más antigua a
  más reciente; Spotify, Tidal y Qobuz no declaran orden. La parada temprana no es
  segura.
- **Versión de playlist:** Spotify (`snapshot_id`), Deezer (`mod_date`) y SoundCloud
  (`last_modified`); Qobuz y Tidal solo conteo; Apple nada. Ninguno identifica qué
  pista salió sin leer los ítems.
- **Comprobación puntual:** Spotify (`/me/tracks/contains`) y Apple (por id de
  biblioteca); el resto no ofrece endpoint de contención.
- **Escritura:** los seis tienen endpoint de like/unlike y cinco ya lo usan en
  migración; Spotify no pide `user-library-modify` en el flujo de autenticación que usa
  la interfaz.
- **Límites:** SoundCloud ~500 peticiones/24 h al endpoint de likes; Spotify ventana de
  30 s por aplicación; Deezer 50 peticiones/5 s; Qobuz, Tidal y Apple no publican cifra
  verificable.
- **Deuda de paginación detectada:** Spotify, Tidal y Qobuz solo piden la primera página
  de álbumes y artistas; Tidal y Qobuz acumulan el total por página (sobreconteo) y
  Deezer lo sobrescribe; SoundCloud no lee `likes_count`.

## Evidencia de la biblioteca local (copia de solo lectura, 2026-11-04)

- 640 playlists y 192.494 pertenencias.
- 64 grupos con el mismo nombre normalizado dentro de la misma cuenta y servicio: 16 con
  el mismo conjunto exacto de pistas y 48 con conjuntos distintos.
- 69 pares con ≥90 % de solapamiento (Jaccard) dentro del mismo nombre.
- 20 grupos con el mismo nombre y conjunto exacto en más de un servicio, con 60 playlists
  implicadas; 137 nombres repetidos entre servicios.
- 3 playlists vacías.
- `playlist_sources` tiene 640 filas y ningún `snapshot_id` escrito.
- `playlists.source_service` está poblado en las 640, pero no se guarda el id de la
  playlist de origen de una migración.


## Consecuencias

- El contrato de sincronización queda cerrado en esta sesión; el siguiente paso es
  la especificación y los tickets, no más decisiones de contrato.
- `incremental_sync` deja de ser un no-op o se retira: con barrido completo, el ahorro
  viene de las huellas de playlist (A-02), no de un interruptor que nadie lee.
