//! Contrato de sincronización (ADR-0006, #64/#65).
//!
//! Dos seams, según la decisión de pruebas del spec:
//!
//! - **Seam 1 — job** ([`job`]): la corrida `SyncRun` con identidad, estado,
//!   progreso, cancelación e historial. Se prueba contra [`fake`]
//!   (`FakeLibrarySource`) y BD `:memory:`, sin red.
//! - **Seam 2 — adaptador** ([`source`]): la interfaz única `LibrarySource`
//!   que los proveedores implementan. Qobuz entra primero
//!   ([`qobuz_source`]); el resto de proveedores siguen por el camino antiguo
//!   hasta sus tickets.

pub mod collection;
pub mod events;
pub mod fake;
pub mod job;
pub mod qobuz_source;
pub mod runs;
pub mod source;
pub mod spotify_source;
pub mod tidal_source;

pub use collection::{collect_all_pages, collect_cursor_pages, CollectedCollection};
pub use events::SyncRunEvent;
pub use qobuz_source::QobuzLibrarySource;
pub use source::{CollectionPage, LibrarySource, SourceErrorKind};
pub use spotify_source::SpotifyLibrarySource;
pub use tidal_source::TidalLibrarySource;
