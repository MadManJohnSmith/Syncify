# Docs de dominio

Cómo deben consumir los skills de ingeniería la documentación de dominio de este
repo al explorar el código.

## Este repo

- **Layout: single-context.** Un único `GLOSSARY.md` en la raíz. No hay
  `GLOSSARY-MAP.md` ni glosarios por contexto.
- **Las decisiones viven en `docs/adr/`**, en ficheros numerados por fecha
  (`NNNN-tema.md`) con la plantilla ADR (título, resumen de una a tres frases y
  estado). Es el único
  registro de decisiones: no crees ficheros de decisión en otro sitio. Los antiguos
  `docs/DECISION_*.md` son punteros de tres líneas que se conservan solo para que
  las citas de `docs/historico/` y `docs/auditoria/` resuelvan.
- **`docs/historico/` es evidencia, no estado actual.** Ahí están handoffs
  fechados, registros de fallos de test, notas de rendimiento y release notes.
  Nunca lo leas como la descripción de cómo se comporta el sistema hoy.
- **`docs/Deuda_Tecnica_y_UX.md` es el registro vivo de deuda.** Un trabajo que lo
  contradiga debe citar el registro que cierra (`D-xx`, `C-xx`) o el que añade.
- **`docs/README.md` es el índice** y declara qué documento es la referencia
  vigente de qué. Si no sabes dónde vive algo, empieza ahí.

## Antes de explorar, lee esto

- **`GLOSSARY.md`** en la raíz: vocabulario de dominio (track vs. track source vs.
  edición, y los dos sentidos de «migración»).
- **`docs/adr/*.md`**: las decisiones que restringen el área en la que vas a
  trabajar, en el orden de sus fechas.

Si alguno de estos ficheros no existe, **sigue en silencio**. No señales su
ausencia ni propongas crearlos de entrada. El skill `/domain-modeling` (al que se
llega vía `/grill-with-docs` y `/improve-codebase-architecture`) los crea de forma
perezosa cuando de verdad se resuelven términos o decisiones.

## Usa el vocabulario del glosario

Cuando tu salida nombre un concepto de dominio (en el título de un issue, una
propuesta de refactor, una hipótesis, el nombre de un test), usa el término tal
como lo define `GLOSSARY.md`. No derives a sinónimos que el glosario evita
explícitamente.

Si el concepto que necesitas aún no está en el glosario, es una señal: o estás
inventando lenguaje que el proyecto no usa (reconsidéralo) o hay un hueco real
(anótalo para `/domain-modeling`).

## Señala conflictos con decisiones

Si tu salida contradice una decisión existente, sácalo explícitamente en vez de
pisarla en silencio:

> _Contradice `docs/adr/0003-seleccion-de-edicion.md`, pero merece reabrirse porque…_

Cita el fichero, no un número inventado: este repo no numera las decisiones.
