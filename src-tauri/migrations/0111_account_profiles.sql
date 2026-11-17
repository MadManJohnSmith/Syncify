-- Migration 0111: perfiles de webview por cuenta e intentos de login
-- (issue #86, ADR-0007 decisión 2).
--
-- `account_profiles` — UN perfil de webview por cuenta. El directorio de
-- datos del webview vive CIFRADO EN REPOSO (AES-256-GCM, la misma clave que
-- las credenciales) en el archivo que señala `archive_path`; se descifra a un
-- directorio temporal SOLO mientras la ventana de login está abierta y se
-- vuelve a cifrar al cerrarla. `account_id` es nulo durante un intento de
-- login: el `profile_key` se crea antes que la cuenta y se vincula al
-- completarla. `key_id` es la huella de la clave usada al cifrar: diagnóstico
-- de «base restaurada en otra máquina» (decisión 10 del ADR).
--
-- `login_attempts` — registro de intentos: la fila nace 'pending' al abrir la
-- ventana y termina 'completed' (vinculando el perfil a la cuenta creada o
-- actualizada) o 'failed'. Cancelar BORRA el intento y su perfil provisional
-- (contrato de #84 y de la spec #83): no queda fila de intentos cancelados.
--
-- Cerrar sesión y eliminar borran el archivo; desactivar y archivar (ciclo de
-- vida de la migración 0108) lo conservan. El borrado del FICHERO en disco es
-- explícito en el comando: la FK aquí solo garantiza que no queden filas
-- huérfanas si una vía no pasa por el comando.

CREATE TABLE account_profiles (
    id INTEGER PRIMARY KEY,
    account_id INTEGER REFERENCES accounts(id) ON DELETE CASCADE,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    profile_key TEXT NOT NULL UNIQUE,
    archive_path TEXT NOT NULL,
    key_id TEXT,
    status TEXT NOT NULL DEFAULT 'archived'
        CHECK (status IN ('archived', 'unrecoverable')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_account_profiles_account_id ON account_profiles(account_id);
CREATE INDEX idx_account_profiles_service_id ON account_profiles(service_id);

CREATE TABLE login_attempts (
    id INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    profile_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'completed', 'failed')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    finished_at TEXT
);

CREATE INDEX idx_login_attempts_profile_key ON login_attempts(profile_key);
