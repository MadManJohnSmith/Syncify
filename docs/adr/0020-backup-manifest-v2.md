# ADR-0020 — Backup: manifest v2 con ajustes y cuentas sin secretos

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo AJUSTES/BACKUP.
**Spec:** #195 · **Tickets:** #196–#202.

## Contexto

ADR-0007 (cláusula 11) promete que el backup incluye «biblioteca, ajustes y
cuentas» sin secretos, pero el manifest v1 solo exporta biblioteca (tracks,
álbumes, artistas, playlists): ni ajustes ni cuentas. Restaurar en otra máquina
hoy pierde toda la configuración. No hay backup programado ni historial en la UI;
las copias internas pre-mutación (VACUUM INTO) no son backup de usuario. La
`schema_version` del manifest está hardcodeada y no refleja el estado real.

## Decisión

1. **Manifest v2.** Incluye la biblioteca como v1, más todas las superficies de
   ajustes (tablas tipadas y KV de ajustes) y metadatos de cuenta (servicio,
   identidad externa, nombre) **sin ningún secreto**: sin tokens de cuenta, sin
   claves de API, sin credenciales de fuentes de letras. Restaurar en otra
   máquina pide reautenticar, como ya dice la cláusula 10.

2. **Versionado honesto.** La `schema_version` del manifest se deriva del estado
   real del esquema, no de una constante. El import de manifests v1 sigue
   aceptándose.

3. **Backup manual formalizado.** Sin programador ni historial en UI: export e
   import manuales con las mismas reglas de sandbox y checksum. La copia
   pre-mutación se documenta como mecanismo interno de recuperación, no como
   backup.

4. **Los ficheros de audio siguen fuera.** El manifest conserva el flag de
   descarga por pista; el audio no viaja en el backup.

## Consecuencias

- Mover de máquina pasa a reproducir biblioteca **y** configuración, con login
  nuevo para las cuentas.
- La exclusión de secretos es contrato testeable: ningún valor del manifest puede
  ser un token o credencial.
- El import v2 debe validar ajustes por superficie y fallar seguro ante
  versiones futuras.

## Alternativas descartadas

- **Enmendar ADR-0007** para que el ADR diga "solo biblioteca". Menos trabajo,
  pero deja roto el traslado de máquina que el flujo quiere resolver.
- **Backup del fichero .db completo user-facing.** Cubriría todo, pero
  transporta secretos (contradice la cláusula sin-secretos) y arrastra riesgo de
  versión de esquema entre máquinas.
- **Auto-backup con retención e historial.** Decidido fuera de alcance: valor
  real, pero nueva superficie de política de retención y disco.
