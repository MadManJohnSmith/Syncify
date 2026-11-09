//! Contrato único de los puentes Python.
//!
//! Un puente es un proceso Python que habla un envelope JSON por stdout
//! (`{"success", "data", "error"}`). Este módulo es la única vía para
//! lanzarlos y para interpretar su salida: antes había tres lanzadores
//! (genérico, auth y settings) y cuatro structs de envelope idénticos.

pub mod client;
pub mod envelope;
pub mod error;

pub use client::{BridgeClient, CallOptions};
pub use envelope::BridgeEnvelope;
pub use error::BridgeError;
