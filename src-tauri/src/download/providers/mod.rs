//! Adaptadores de proveedor y los motores que envuelven.
//!
//! Cada fichero declara un trait de motor pequeño y object-safe (el descargador
//! real implementa ese trait) y el adaptador [`DownloadProvider`] que traduce
//! petición, errores, enrutado y calidad. Los motores son sustituibles en los
//! tests sin red.
//!
//! [`DownloadProvider`]: crate::download::provider_gateway::DownloadProvider

pub mod amazon;
pub mod qobuz;
pub mod tidal;

pub use amazon::AmazonProvider;
pub use qobuz::QobuzProvider;
pub use tidal::TidalProvider;
