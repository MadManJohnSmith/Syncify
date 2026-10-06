-- Migration 0089: identidad de cuenta por `external_user_id` + invariante de
-- una sola cuenta activa por servicio + normalización de emails.
--
-- Contexto (multi-cuenta). El login resolvía la fila destino del servicio por
-- `email IS ?` y, si no coincidía, TOMABA LA FILA MÁS NUEVA y le sobrescribía
-- email/credenciales (commands/auth.rs). Con dos cuentas del mismo servicio eso
-- destruye la biblioteca de la anterior (CASCADE sobre library_entries y
-- playlists) y hace indistinguibles dos sesiones del mismo usuario.
--
-- (a) `external_user_id`: identidad estable que devuelve el proveedor (spotify
--     `id`, tidal `user_id`). Columna NULLABLE → retrocompatible con las filas
--     ya guardadas y con UNIQUE(service_id, email) de 0002, que NO se toca.
--
-- (b) idx_accounts_service_external_user: índice PARCIAL (solo external_user_id
--     NOT NULL). Impide dos filas del mismo servicio con el mismo user_id sin
--     afectar a las filas NULL (que es el caso de deezer/soundcloud y de lo que
--     no ha vuelto a logaritharse desde esta migración).
--
-- (c) Repair one-shot de la exclusividad: 'exactamente una activa por servicio',
--     con la MISMA regla determinista que usan todos los resolvedores del
--     backend (`is_active = 1 ORDER BY id DESC LIMIT 1` → sobrevive MAX(id)).
--     Es un no-op con una sola cuenta por servicio; elimina el estado hoy
--     posible de ≥2 activas (toggle_account_active, add_account y las filas
--     sintéticas de import_service podían dejarlo así).
--
-- (d) Normalización de emails SEGURA ante la UNIQUE BINARY de 0002: 'A@x.com' y
--     'a@x.com' pueden coexistir, y normalizar solo una de las dos crearía una
--     pareja nueva. Por eso la detección de colisión usa EXACTAMENTE la misma
--     expresión que el matching en runtime (`LOWER(TRIM(...))` en AMBOS lados):
--     cuando existe una hermana case-variante la fila NO se normaliza y las dos
--     quedan como están; se normaliza únicamente cuando no colisiona.
--     NO se fusionan parejas case-variantes: fusionar decidiría qué
--     biblioteca/playlists (CASCADE) sobrevive, y eso es una decisión de datos
--     que no corresponde a una migración de esquema. El matching en runtime
--     sigue siendo case-insensitive y determinista (ORDER BY id DESC LIMIT 1),
--     así que esos residuos no afectan a la identidad de las cuentas.
--
-- Sin backfill de external_user_id: lo rellena el próximo login de cada cuenta.

ALTER TABLE accounts ADD COLUMN external_user_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_service_external_user
    ON accounts(service_id, external_user_id)
    WHERE external_user_id IS NOT NULL;

-- (c) Repair: una sola cuenta activa por servicio (la de id más alto).
UPDATE accounts
SET is_active = 0
WHERE is_active = 1
  AND id NOT IN (
      SELECT MAX(id) FROM accounts WHERE is_active = 1 GROUP BY service_id
  );

-- (d) Normalización de email solo cuando no colisiona con una hermana.
UPDATE accounts
SET email = LOWER(TRIM(email))
WHERE email IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM accounts dup
      WHERE dup.service_id = accounts.service_id
        AND dup.id != accounts.id
        AND LOWER(TRIM(dup.email)) = LOWER(TRIM(accounts.email))
  );