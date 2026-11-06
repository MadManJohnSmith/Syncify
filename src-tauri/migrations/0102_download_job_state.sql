-- P4 (ADR-0022): la máquina de estados de descargas persiste la clase de fallo
-- y la próxima ventana de reintento automático, para que el veredicto se
-- recalcule desde la fila tras un reinicio en vez de desde un centinela.
ALTER TABLE download_queue ADD COLUMN failure_class TEXT;
ALTER TABLE download_queue ADD COLUMN next_attempt_at TEXT;

-- El claim del worker filtra por estado y vencimiento del reintento.
CREATE INDEX IF NOT EXISTS idx_download_queue_retry
    ON download_queue(status, next_attempt_at);
