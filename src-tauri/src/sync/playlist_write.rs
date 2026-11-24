//! Gateway de escritura de playlists para Qobuz y Tidal (PL06, #159).
//!
//! El seam público es `PlaylistWriteGateway`: valida la petición, consulta las
//! capacidades declaradas por el proveedor y delega en `PlaylistWriteProvider`.
//! Los ids son cadenas opacas dentro de un proveedor; el gateway no los mezcla
//! con ids locales ni con ids de otro servicio.
//!
//! ## Capacidades verificadas en el árbol
//!
//! Los clientes existentes ya escriben `playlist/create` y `playlist/addTracks`
//! en Qobuz, y `POST /users/{id}/playlists` y `POST /playlists/{id}/items` en
//! Tidal. Son las únicas operaciones que este gateway promete hoy. Los endpoints
//! de actualización, retirada de ocurrencias, reordenación y borrado remoto no
//! están cableados en esos clientes: se declaran `Unsupported` en vez de
//! simularlos con escrituras locales.
//!
//! ## Errores y reintentos
//!
//! 401, 403, 429, timeout y validación son clases distintas. Clasificar un fallo
//! no autoriza a reintentarlo: crear y añadir no son idempotentes, por lo que un
//! timeout puede haber dejado una escritura parcial en el proveedor. La cola de
//! PL07 decidirá la reconciliación antes de repetir esas operaciones.
//!
//! ## Pruebas
//!
//! `FakePlaylistWriteProvider` mantiene el estado en memoria y programa fallos
//! por operación. Las pruebas del gateway no abren sockets ni consultan servicios
//! reales.

#[cfg(test)]
mod tests {
    use crate::services::{QobuzClient, TidalClient};

    #[tokio::test]
    async fn existing_clients_treat_an_empty_add_as_a_noop() {
        let qobuz = QobuzClient::new("app".into(), "secret".into());
        qobuz
            .add_playlist_tracks(7, &[])
            .await
            .expect("an empty Qobuz add must not issue HTTP");

        let tidal = TidalClient::new("token".into()).with_user("user".into(), "MX".into());
        tidal
            .add_playlist_tracks("playlist", &[])
            .await
            .expect("an empty Tidal add must not issue HTTP");
    }
}
