# Handoff — Syncify: flujos de documentación

**Estado:** vivo · **Última revisión:** 2026-11-05

Fecha: 2026-11-05. Rama: `dev`.
Tracker: GitHub Issues de MadManJohnSmith/Syncify vía gh.

## Objetivo

Continuar el flujo por cada flujo de la app: /grill-with-docs → /to-spec → /to-tickets,
hasta cubrir prebeta y postbeta. El siguiente flujo es DESCARGAS.

## Hecho (referencias, no duplicar)
- Documentación en GLOSSARY.md, docs/adr/, docs/agents/; puerta test_docs_consistency verde.
- ADR-0001..0003 migrados; ADR-0004 y ADR-0005 propuestos.
- Sync: ADR-0006, spec #64, tickets #65-#82.
- Cuentas: ADR-0007, spec #83, tickets #84-#95; login #62, sonda #63.
- Biblioteca: ADR-0008, spec #96, tickets #97-#110.

## Mapa de issues y dependencias
- Sync: T01=#65 ... T18=#82 (T08 cuarentena #72, T09 identidad #73, T15 reauth #79, T16 metadatos #80).
- Cuentas: A01=#84 ... A12=#95; A04 (reauth) depende de #79.
- Biblioteca: L00=#97 (pertenencia por colección/cuenta; depende de #65-#68), L01=#98 (consulta v2; depende de #97 y #88), L02..#L13=#99-#110.

## Pendiente
- DESCARGAS: grill pendiente. El subagente que iba a mapear el flujo fue cancelado; no hay hechos frescos.
- Leer docs/adr/0004, docs/adr/0005, docs/features/download-pipeline.md y mapear UI, worker, resolver, entitlements, verificación y proveedores.
- Después: reproductor, playlists, migración, letras, ajustes/backup y postbeta (grupos A-E de docs/PLAN_MEJORA_POST_BETA.md).

## Gotchas
- Descargas: ADR-0004 registra 17.969 favoritos, preflight + pasada real, modal no cancelable y 350 fallos/hora (308 Tidal stream resolution).
- ADR-0005 propone download_entitlements y reutilizar evidencia de entregas; verificar el resolver actual antes de preguntar.
- Biblioteca: library_items es espejo de migración; library_stats cuenta filas; auto-fusión de duplicados contradice ADR-0008; playlists truncadas a 500; búsqueda en letras postbeta.
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
- grill-with-docs (grilling + domain-modeling) para descargas.
- to-spec y to-tickets para publicar y dividir.
- ask-matt si hay duda de skill; domain-modeling para glosario/ADR.

## Siguiente paso concreto
1. Leer ADR-0004, ADR-0005 y download-pipeline.md; mapear el código de descargas.
2. Ronda 1 de grill: calidad (tier vs exacta, fallback), contrato de cola (id, progreso, pausa/cancelar/reanudar), enrutado por cuenta (evidencia, TTL, negative cache) y verificación de éxito en disco.
3. Escribir ADR-0009 (o ampliar 0004/0005) y seguir con to-spec/to-tickets.
