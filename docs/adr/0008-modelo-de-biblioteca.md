# ADR-0008 — Modelo de biblioteca: alcance, navegación y detalle

**Estado:** propuesto · **Fecha:** 2026-11-04
**Origen:** ronda de grilling del flujo de biblioteca, con 31.359 pistas y 14 cuentas.
**Spec:** #96 · **Tickets:** #97 a #110.

## Contexto

La biblioteca actual es una sola lista virtualizada sobre todas las pistas del catálogo.
No muestra la procedencia por cuenta, no tiene vista de cuarentena ni de duplicados, el
detalle vive en otra pantalla (`/metadata`), el artista solo enseña sus cinco primeras
pistas y las playlists se truncan a 500. El scroll llegó a disparar 319 llamadas a
`get_library` en cuatro minutos y el orden por «fecha añadida» no compara nada.

## Decisión

1. **La biblioteca incluye todo lo importado de cualquier colección, menos la
   cuarentena.** Favoritos, álbumes, playlists y artistas; una pista que solo vive en una
   playlist también aparece. La cuarentena se excluye por defecto de listas y contadores.
2. **La navegación tiene pestañas de tracks, álbumes y artistas.** La vista de biblioteca
   gana esas tres pestañas; playlists y descargas mantienen sus vistas actuales. Álbumes y
   artistas dejan de ser solo un detalle: tienen listas navegables.
3. **El detalle de pista es un panel lateral con enlace a la ficha completa.** El panel
   muestra fuentes, cuentas, descarga, calidad y apariciones; «abrir ficha» lleva a
   Metadata para editar.
4. **«Seleccionar todo» significa todo el filtro actual, no la página cargada.** El
   checkbox superior muestra el recuento y las acciones se ejecutan en servidor por lotes.

5. **La búsqueda cubre título, artista, álbum, género e ISRC con relevancia.** En beta
   se indexan esos campos y se combinan con los filtros; la búsqueda en letras queda para
   post-beta.
6. **El orden por defecto es fecha añadida descendente, con comparador real.** Se corrige
   el comparador muerto y se desempata por título y artista para que el orden no baile.
7. **Los duplicados tienen vista propia por ISRC y huella.** Se agrupan, se muestra el diff
   y la fusión es manual y reversible; nunca automática.
8. **La cuarentena es un filtro «ya no está».** Excluida por defecto de listas y
   contadores, con su motivo; si el origen vuelve, se restaura sola.
9. **La pestaña de álbumes lista álbumes con artista, año, pistas, cobertura de descarga y
   cuentas.** Al abrir un álbum se ven sus pistas y acciones.
10. **La pestaña de artistas lista artistas con pistas, álbumes y apariciones.** El detalle
    muestra todas las pistas del artista, no solo cinco, y sus apariciones cross-service.
11. **Las descargas se integran con badge, filtro y acciones.** La pista muestra estado,
    calidad y ruta; descargar, borrar audio y re-descargar se hacen desde ahí. Borrar audio
    no saca la pista de la biblioteca.
12. **Las playlists se integran con badge y filtro «en playlist».** La gestión sigue en su
    vista; la biblioteca solo muestra pertenencia y permite filtrar.
13. **Las acciones masivas son descargar, marcar favorito, añadir a playlist y editar
    metadatos.** Borrar audio y tocar la cuarentena quedan por pista y con confirmación.
14. **Los metadatos se editan en la ficha completa, no en el panel lateral.** Los campos
    editados a mano se bloquean frente al sync y las contradicciones van a `needs_review`.
15. **Todas las vistas se virtualizan y la búsqueda, el orden y los filtros van al
    servidor.** Lista, grid y grupos; FTS e índices; ninguna fila dispara consultas por
    pista ni el orden se calcula en JavaScript.
16. **La cabecera muestra el total de pistas distintas y chips accionables** de álbumes,
    artistas, descargadas, cuarentena y duplicados; cada chip aplica su filtro.


## Consecuencias

- Las consultas de la biblioteca pasan a agregar por pista canónica y por cuenta; el
  detalle necesita una consulta por pista con fuentes, cuentas y descargas.
- Las pestañas de álbumes y artistas requieren rutas o subrutas y listas paginadas.
- La selección masiva obliga a comandos por lote y a un recuento exacto antes de ejecutar.
  debe implementarse de verdad.

## Preguntas abiertas

No quedan decisiones de contrato para la biblioteca. La gestión de playlists (límite
de 500, smart playlists y clasificación local/importada) se trata en su propio flujo;
la búsqueda en letras queda para post-beta.
