//! Un módulo por Fuente de letras (ADR-0017): cada fichero encapsula el
//! fetcher de su fuente; la política y la selección viven en `crate::client`.

pub mod apple_music;
pub mod kugou;
pub mod lrclib;
pub mod lyricsplus;
pub mod musixmatch;
pub mod netease;
pub mod qqmusic;
pub mod spotify;
pub mod tekstowo;
pub mod ultrastar;
