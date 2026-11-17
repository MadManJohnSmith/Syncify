-- Migration 0107: ciclo de vida explícito de cuenta (issue #85, ADR-0007 decisión 3)
--
-- `accounts.state` pasa a ser el ciclo de vida: 'enabled' | 'disabled' | 'archived'.
-- Eliminar no es un estado: la fila desaparece. El criterio de aceptación del
-- issue: la cuenta que hoy está activa (`is_active = 1`) queda 'enabled' en el
-- upgrade; las demás también se habilitan (el ADR lo dice así: "habilita las
-- demás"), y una credencial inválida NO es un estado del ciclo de vida: vive en
-- `credentials_invalid`, que ya lleva su propia marca de reautenticación.
--
-- `is_active` NO se retira aquí: la retirada completa cruza la consulta de
-- biblioteca (carril #99) y los resolvedores legacy; se apaga por partes en los
-- tickets siguientes de la serie. El mapping del contrato (ADR-0007, regla
-- derivada): 'disabled' y 'archived' implican no participar (is_active = 0),
-- lo aplica el comando, no esta migración.

ALTER TABLE accounts ADD COLUMN state TEXT NOT NULL DEFAULT 'enabled'
    CHECK (state IN ('enabled', 'disabled', 'archived'));

-- Backfill: todo lo existente se habilita; en particular la cuenta activa de
-- cada servicio conserva su papel como habilitada (criterio 4 del issue #85).
UPDATE accounts SET state = 'enabled' WHERE state IS NULL OR state = '';

CREATE INDEX idx_accounts_state ON accounts(state);
