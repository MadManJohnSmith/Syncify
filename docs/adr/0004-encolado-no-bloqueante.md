# ADR-0004 — Encolado de descargas no bloqueante y a escala de biblioteca

**Estado:** propuesto · **Fecha:** 2026-11-04
**Origen:** requisito del propietario durante la verificación manual de la beta, con la
biblioteca real como carga.

## Contexto

La biblioteca medida tiene 31.359 pistas, 88.848 entradas de biblioteca y **17.969
pistas favoritas distintas**. El propietario encoló descargas y la aplicación quedó
bloqueada: a los 20 minutos, el modal seguía en «checking candidates».

El recorrido de `download_favorites` explica el tiempo:

- Itera los candidatos **en serie** y, por cada pista, ejecuta el guardrail de cola,
  busca fallos anteriores en `download_queue`, lee metadatos con subconsultas de
  artista y, fuera del preflight, inserta una fila.
- Con más de 100 candidatas y sin límite explícito, primero corre un **preflight** y
  después la pasada real: el recorrido completo se paga dos veces.
- El preflight no escribe: su guarda está antes del INSERT, así que abortarlo no deja
  filas a medias.
- El modal deshabilita el cierre mientras `isProcessing` está activo y no existe id de
  trabajo, progreso ni cancelación. La única señal es el botón girando.

En paralelo, la cola acumuló **350 fallos en una hora**; 308 son «Tidal pipeline
failed: Stream resolution rejected or unavailable» y la cuenta Tidal activa no tiene
biblioteca propia. La selección de cuenta para descargar es parte del problema, no
solo el volumen.

## Decisión

La cola de descargas **no puede bloquear la interfaz ni crecer linealmente con el
tamaño de la biblioteca**. Debe funcionar con decenas de miles de pistas y dejar que
el usuario siga usando la aplicación mientras encola. Es un requisito del propietario,
no una optimización opcional.

## Decisiones del propietario (ronda 1, 2026-11-05)

- **Propósito:** las descargas sirven tanto para escucha offline dentro de Syncify como
  para exportar a un reproductor externo (Symfonium u otro). Los ficheros deben ser
  exportables: tags completos, portada incrustada y estructura de carpetas estable.
- **Alcance beta:** este ADR y el ADR-0005 son requisitos de beta. De la gestión
  avanzada (prioridades, límites por proveedor, programación) solo se adelanta la
  pausa global.
- **Modelo:** un *job* por acción del usuario (id, tipo, filtro, contadores de progreso)
  agrupa filas de **una única cola persistente**, que sigue siendo la fuente de verdad.
  El job no es un segundo planificador.
- **Cancelar:** cancela los pendientes del job y deja terminar lo que ya está en curso;
  lo insertado se conserva. La pausa global detiene el claim y es reanudable.
- **Favoritos:** «descargar favoritos» sigue siendo acción de primer nivel, como job en
  segundo plano con estimación, progreso y cancelación, sin modal bloqueante.

- **Fin de job:** un job termina como `completed`, `partial` (con contadores de
  completadas, fallidas y omitidas) o `cancelled`; los fallos no bloquean el job y
  siguen visibles para reintento.
- **Arranque:** al reiniciar, las filas pendientes se reanudan solas; las que estaban
  en curso pasan por el journal (completar si el fichero es válido, reencolar una vez
  si no, fallar si es terminal).
- **Estimación:** el recuento para confirmar un job se calcula con consultas agregadas
  al encolar, sin recorrido por pista ni resolución viva por ISRC. El panel muestra el
  progreso real.
- **Confirmación:** los jobs de más de 1.000 candidatos muestran la estimación y piden
  confirmación; los acotados encolan directo. El modal no se bloquea ni espera la
  promesa del comando.
- **Monitor:** los jobs viven en un panel de la vista de Descargas con contadores,
  progreso y cancelación de pendientes; el modal solo crea el job y se cierra.
- **Cancelar en activo:** el botón de cancelar solo aparece en pendientes; en una
  descarga activa se deshabilita con el aviso de que terminará la actual.

## Propuesta, a validar en la ronda de grilling

- Encolar como **trabajo en segundo plano** con identificador, progreso por eventos y
  cancelación explícita. La interfaz deja de esperar la promesa del comando.
- **Preflight con consultas agregadas**, no con un recorrido por pista.
- **Inserción por lotes**: una sentencia con `INSERT ... SELECT` o transacciones por
  bloques, en lugar de un INSERT por pista.
- **Índices** para el guardrail y para la consulta de candidatos; medir con el
  planificador antes y después.
- **Idempotencia**: repetir el trabajo no duplica filas ni posiciones.
- El modal pasa a ser **monitor del trabajo**; cerrarlo no cancela nada.
- **Prueba de carga** con un fixture de 30.000 pistas y un presupuesto de tiempo
  escrito en el test.

## Consecuencias

- Cambio aditivo: una tabla de trabajos o el diario de operaciones existente, sin
  tocar las tablas de biblioteca.
- Eventos nuevos en el contrato IPC (progreso, fin, error, cancelado).
- El preflight deja de costar una segunda pasada completa.
- La cuenta usada para descargar debe elegirse por la fuente que tiene la identidad,
  no solo por «activa»; los 350 fallos de Tidal lo hacen visible.

## Preguntas abiertas

- ¿El preflight sobrevive como paso separado o se fusiona con la ejecución?
- Tamaño de lote por filas y por tiempo.
- ~~Cancelar: ¿se conserva lo ya insertado o se revierte el lote en curso?~~ Resuelto
  (ronda 1): se conserva lo insertado y lo que está en curso termina; la pausa global
  detiene el claim y es reanudable.
- ¿La resolución viva por ISRC de fuentes de catálogo también pasa al worker?
- ¿Qué cuenta se elige cuando varias del mismo servicio pueden tener la pista? Se
desarrolla en el ADR-0005.

## Evidencia

Copia de `syncify.db` del 2026-11-04 17:20 (local): 17.969 favoritos distintos. Una
segunda copia, unos minutos después, muestra la cola en 198 pendientes, 153
completadas, 10 descargando y 350 fallidas, todas de la última hora. El preflight
seguía activo a los 20 minutos con el proceso al 73 % de CPU.
